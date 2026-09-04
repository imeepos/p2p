# worktree 环境变量自动接入（.env）

状态：已启用（2026-09-04）。真实凭据只在各工作树的 `.env`（已 gitignore，不进版本库）。

## 机制

`githooks/post-checkout` 在分支级检出后自动为当前工作树补齐 `.env`：

- 触发时机：新建 worktree / clone 完成时的检出，以及任意 HEAD 级分支切换；
  单文件路径检出（`git checkout -- <path>`）不触发。
- 来源优先级：主工作树 `.env`（真实值）→ 仓库内 `.env.example`（兜底模板，只有键名与占位值）。
- 覆盖保护：当前工作树已存在 `.env` 时一律不读不写，既有内容零改动。
- 自愈：工作树缺 `.env` 时，任意一次 HEAD 级检出都会自动补齐。
- 可观测性：成功与失败都输出 `[env-hook]` 行（stderr），失败不阻断检出本身。

## 一次性引导（全新 clone 后）

```bash
cd <仓库根>
git config core.hooksPath "$(pwd)/githooks"   # 必须绝对路径，原因见下
```

引导后若主工作树还没有 `.env`：`cp .env.example .env` 并填入真实值（仅此一次手工步骤）。
此后 `git worktree add ...` 的新工作树自动带上可用 `.env`；
`core.hooksPath` 记录在本 clone 的仓库配置里，对本 clone 的全部 worktree 生效，无需逐个设置。

## 为什么必须绝对路径

`git worktree add` 触发钩子时，相对形式的 `core.hooksPath` 相对"发起命令时的 cwd"解析：
若从没有检出 `githooks/` 的目录发起（例如钩子目录尚未合并进主树、或从仓库外目录发起），
钩子会被 git 静默跳过，无任何报错。绝对路径消除该歧义，任意 cwd 发起均可靠。

## 验证

```bash
git worktree add --detach .worktrees/envhook-check "$(git rev-parse HEAD)"
# 预期 stderr 出现：[env-hook] 已为 ... 生成 .env (来源: 主工作树 .env)
ls -l .worktrees/envhook-check/.env           # 存在且 0600
git worktree remove --force .worktrees/envhook-check
```

主工作树若无 `.env`（如仅做 CI 检查的 clone），则来源回退为 `.env.example`，
新工作树的 `.env` 为占位值，需填入真实值后才能跑依赖外部服务的用例。
