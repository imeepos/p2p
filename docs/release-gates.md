# 发布门禁（release gates）

> 2026-09-03 起生效。目标：发布路径上的每一步都有机械门禁，且门禁只在其必然经过的路径上才有效。

## 1. 事故复盘（client-v0.1.1 发布，2026-09-03）

### 时间线

| 时间 | 事件 |
| --- | --- |
| 01:52 | `1e03808` chore(release): bump p2p-console 0.1.1——四触点（package.json / tauri.conf.json / Cargo.toml / Cargo.lock）人工同步 |
| ~02:00 | bump 后 `make check` 假红：about-update-card 测试硬编码 v0.1.0（`e377022` 修复，断言改读 `__APP_VERSION__` 与实现同源） |
| ~02:05 | 打 annotated tag client-v0.1.1 并推送，触发 gui-client 流水线发布 Release |

### 根因（三条，本文件的门禁逐条对应）

- **R1 版本多触点人工同步，无机械校验**。版本散在三处声明 + 一处 lock，bump 靠手改；
  漂移不会被任何门禁拦截，只会在线上更新检查（W7 currentVersion 取 tauri.conf）里静默错位。
  假红事故是同一根因的另一面：断言硬编码版本，bump 后门禁红但非真实回归，逼迫人工判断"这次红是不是真的"。
- **R2 CI gate 只挂在 pull_request 路径上**。直接 push main（含 release bump 提交）完全绕过 gate，
  未 lint/未测的代码可以直接进 main。
- **R3 tag 发布无锚定校验**。gui-client.yml 的 tag 触发不检查 tag 指向的 commit 是否在 main 历史内，
  checkout fetch-depth=1 也做不了祖先判定；本地打 tag 前同样无门禁（脏树、版本不一致照发）。

### 教训

- 门禁必须覆盖"必然经过的路径"：只在 PR 上挂 gate，等于放行直接 push。
- 发布是幂等敏感操作：tag 命令必须由校验通过后的脚本打印/执行，不靠人从历史消息里抄。
- 假红与漏报同样危险：假红训练出"红了自己修断言"的肌肉记忆，真红时就会漏。

## 2. 门禁分层

| 层 | 入口 | 校验内容 | 失败行为 |
| --- | --- | --- | --- |
| L0 | `scripts/check/version.sh` | 三处版本同值；带参时须等于参数 | stderr `version-check: FAIL` + 三处实际值，exit 1 |
| L1 | `make check` | L0 + 格式 + 行数 + clippy + 测试 + GUI（lint/build/vitest/冒烟） | 首个失败环快失败 |
| L2 | `make release-check` | main 分支（`RELEASE_ALLOW_BRANCH=1` 可绕过，供 CI/测试）+ 干净树 + L0 + L1 | 逐项给出具体原因 |
| L3 | `scripts/check/release.sh <v> [--create]` | semver + 版本等于参数 + 干净树 + tag 本地/远端不存在 | 拒绝并打印原因；远端不可达时显式 WARN |
| L4 | CI gate（gui-client.yml） | main push 即跑（含 L0 + lint/test/build/i18n） | gate 红不进打包矩阵 |
| L5 | CI tag 门禁 | fetch-depth 0 + tag commit 必须 `git merge-base --is-ancestor` 于 origin/main | tag 不在 main 上则整个流水线红 |

设计取舍：

- **L4 不做 push paths 过滤**。GitHub Actions 中 paths 与 tags 过滤同块时，tag 事件可能因"无新增提交差异"
  被静默滤掉——发布不可静默失败，宁可 gate 对全部 main push 运行（apps/gui 相关 push 是其超集）。
  main push 只跑 gate（build job 带 `if` 条件），不占三平台矩阵。
- **L3 不含 make check / 分支校验**。那是 L2 的职责；release.sh 保持秒级可跑，
  ``--create`` 也只打本地 tag、永不自动 push（push 是最后的不可逆动作，永远人工执行）。
- **Cargo.lock 是第四触点但不在 L0**：它由 cargo 构建（`cargo check`）再生成，属派生物；
  提交 bump 时记得跑一次让 lock 跟上（`1e03808` 先例即四触点同提交）。
- **测试钩子**：两个脚本均支持 `CHECK_ROOT` 覆盖仓库根，release.sh 另支持 `RELEASE_SKIP_REMOTE=1`，
  自测夹具据此在临时目录里驱动成功/失败路径，不碰真实仓库。

## 3. 发布流程（runbook）

前置：功能已合并 main（经 PR 触发的 L4 gate），本地主树在 main 且与远端一致。

1. **bump 版本**：三处（`apps/gui/package.json`、`apps/gui/src-tauri/tauri.conf.json`、
   `apps/gui/src-tauri/Cargo.toml`）同值改版本，`cargo check` 刷新 Cargo.lock，提交
   `chore(release): bump p2p-console <version>`。
2. **推 main**：push 后 L4 gate 自动跑一遍（版本一致 + lint/test/build/i18n）。
3. **总门禁**：主树上 `make release-check`——分支/干净树/版本/全量检查一次过。
   红了先修，不许带红发布。
4. **校验并取命令**：`bash scripts/check/release.sh <version>`——semver、版本等于参数、
   干净树、tag 未占用全部通过后，打印确切的 tag 命令（默认不执行）。
5. **打 tag**：`bash scripts/check/release.sh <version> --create`——本地创建 annotated tag。
6. **推 tag**：`git push origin client-v<version>`（人工执行，最后一步不可逆动作）。
7. **CI**：L5 校验 tag commit 在 origin/main 上 → 三平台打包 → 自动发 GitHub Release。

回滚：远端 `git push origin :refs/tags/client-v<version>` 删 tag（Release 需在 GitHub 页面另删）；
已装客户端靠 W7 更新检查回退到上一版本。

## 4. 自测

```bash
bash scripts/check/tests/release-gates.sh
```

覆盖清单（全部断言退出码 + 输出标记双条件）：

- version.sh：三处一致通过 / 期望版本匹配 / 期望不匹配拒绝 / 三处不一致拒绝 /
  文件缺失拒绝 / 真实仓库当前一致；Cargo.toml 夹具带内联依赖 `version = "2"` 验证不误读。
- release.sh：缺参 / 未知参数 / 非 semver / 版本不等于参数 / 脏树 / 本地 tag 已存在 /
  远端 tag 已存在（裸仓库当 origin）/ 默认只打印命令不创建 tag /
  `--create` 生成指向 HEAD 的 annotated tag / 远端不可达时 WARN 不拦。

夹具全部在 `mktemp -d` 临时目录内构造（含 git 仓库与 file 协议 origin），退出即清理。
