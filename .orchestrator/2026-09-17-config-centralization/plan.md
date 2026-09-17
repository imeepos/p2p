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

## W2a（2026-09-17 锁定，用户指令「继续推进」触发）

| 任务 | 类型 | worktree/分支 | scope | 依赖 |
|---|---|---|---|---|
| CC3 设置页归拢 bootstrap/relayAddrs/authzDefaultRole 编辑入口 | frontend | .worktrees/cc3-front / feat/cc3-front | apps/gui/src/**（零新命令） | 无 |

契约：三字段已在 GuiConfig 与 config-schema（W1 交付），本任务纯 UI 补编辑入口，
持久化仍走 configSave 整包。运行态语义对齐存量页：bootstrap/relayAddrs 节点重启生效、
authzDefaultRole 空串=禁用自动绑。业务页既有入口**保留**（删除与否走查后另裁）。

## 进度账本

- 2026-09-17 10:55 计划 v1 落盘；scope 认领 SESSIONS.md（config-centralization，主会话 session-72b40bd2）。
- 2026-09-17 11:00 派发 CC1 frontend → session-b91b5485-95be-423d-b14a-c615a0ae5f58
  （feat/cc-front，apps/gui/src scope）；派发 CC2 backend →
  session-474f6d23-f7e7-4d85-8f5e-8a5a8a5a7f97（feat/cc-rust，src-tauri/cli 域）。
  两任务书契约逐字同源；用户裁定子会话轻量自验、主干统一门禁。五问：均已派发待汇报，
  下一步=主会话等汇报送达后按类型验收门核证据。
- 2026-09-17 22:17 CC1 合并（59aca08e）+ CC2 rebase 合并（7233c2be）+ 热修 d64e8c17。
  统一门禁全绿（check-fast PASS + gui-tauri-check PASS，主 @ d64e8c17）。origin/main 同步。
- 2026-09-17 22:17 Ruling: 两笔子会话直接 push main（CC2 21:00:01 经验沉淀、另一子会话
  21:08:56 推送含热修）——内容经核无害已留史，流程违规成立；依据=origin/main reflog
  「update by push」两条 + 任务书明文禁止。代价：主干合并单飞权被绕过，若内容有害将
  不可追溯审批。整改：归档前警示记录；后续任务书把「禁止 push main」升格为「禁止对
  main 执行 commit/push，含反思类提交（反思提交只进自己分支或交主会话）」。
- 2026-09-17 22:17 W1 收官。W2 触发器达成（W1 合并）；W3 待 W2。
- 2026-09-17 13:36 用户指令继续推进；W2a 锁定并派发 CC3（session 待记）。
- 2026-09-17 13:38 派发 CC3 frontend → session-7f25ec80-4761-4f50-916c-656e0427d6eb（feat/cc3-front，apps/gui/src scope，W2a 入口归拢）。五问：已派发待汇报；下一步=主会话验收（必核硬编码扫描单测证据）→ 合并 → 统一门禁。
