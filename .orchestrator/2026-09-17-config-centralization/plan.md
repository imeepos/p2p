# 配置集中化波 · 计划 v1（2026-09-17）

> 触发器：用户指令「remote-access 这些都是配置信息，都应该放配置页面」+「立即安排任务，尽量并行，子会话不做复杂构建及错误检查，统一到主会话检查」。
> 盘点真值源：docs/design/config-centralization.md（main@95e78a23）。

## 目标

散落在设置页之外的持久化配置集中进设置页；运行态操作留业务页（判定原则见盘点文档 §2）。

## 锁定区（本波次 W1）

| 任务 | 类型 | worktree/分支 | 文件 scope（零重叠） | 依赖 |
|---|---|---|---|---|
| CC1 设置页远程访问区 + rd 卡读默认 | frontend | .worktrees/cc-front / feat/cc-front | apps/gui/src/**（settings、remote-access、ipc-types.ts、i18n） | 无（契约钉任务书） |
| CC2 GuiConfig 扩展与装配消费 | backend | .worktrees/cc-rust / feat/cc-rust | apps/gui/src-tauri/**、apps/cli/**、docs/design/gui-contract.md | 无（契约钉任务书） |

**契约（两任务书逐字携带，唯一仲裁源）**：GuiConfig 新增三字段（serde camelCase）
- `rdRequireApproval: bool`，缺省 true
- `rdFps: u8`，缺省 15（合法域 1..=60）
- `tunnelServeAllow: Vec<String>`（"127.0.0.1:<port>" 字面量），缺省空

**合并序**：先完工先合（ff-only，主会话执行主干合并）；后合者回自己 worktree `git rebase origin/main` 再试（两树零文件重叠，预期无冲突）。
**检查分工（用户裁定）**：子会话只做轻量自验（聚焦测试 + cargo check/tsc）；全量 make check / gui-check / gui-tauri-check 由主会话在主干统一跑。

## 资源预分配

- 迁移号：本波无 DB 迁移。
- i18n 键：`settings.remoteAccess.*` 命名空间归 CC1 独占；独立 chore(i18n) 小提交。
- 契约章节：gui-contract §3 GuiConfig 字段表归 CC2 独占更新。
- cli-parity：本波**不新增命令**（复用 config_get/config_save、rd_host_start、tunnel_serve_start），无登记变更。

## 待细化区（远期，触发条件）

- W2（bootstrap/relayAddrs/authzDefaultRole 设置页入口归拢 + FTP/static-peers GUI 面）：触发 = W1 合并收官。
- W3（ACP endpoint token 明文 localStorage → 后端存储）：触发 = W2 收官；触安全敏感，实施前出决策备忘录。
- 裁决挂账：FTP 账号表与 static-peers 的 GUI 暴露面需用户点头（盘点文档 §6.3）。

## 账本

进度账本 = 本文件「进度账本」节，每事件一行（五问裁决）；任务账本 = 各任务书 + 汇报。

## 进度账本

- 2026-09-17 10:55 计划 v1 落盘；scope 认领 SESSIONS.md（config-centralization，主会话 session-72b40bd2）。
