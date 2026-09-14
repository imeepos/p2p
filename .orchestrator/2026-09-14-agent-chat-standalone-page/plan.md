# 2026-09-14 agent 独立会话页（agent-chat-standalone-page，ACS）

> 波次账本：Facts / Guesses 分离，Guesses 带过期条件；计划版本变更记日志。

## 目标（用户裁决 2026-09-14 15:57）

本机 agent 界面参照 DSH 会话界面做成独立页面；agent 会话唯一入口迁出 /chat。

用户三裁决：
1. 独立页建成后 /chat 移除 agent 条目（唯一入口）。
2. 本波范围=基础盘：三区布局 + 专属会话侧栏 + 对话流 + composer。
   任务清单卡片（ACP plan）、对话/轨迹双 tab、附件/模型选择器均不在本波。
3. 覆盖所有 ACP 端点（本机 + 远端），组件同一套。

## Facts（已核代码 2026-09-14）

- agent 会话现状：/chat 双栏右栏一种形态（`?agent=` 深链，AgentConversation）。
- 数据面就绪：acp-store 有 `sessions: SessionSummary[]`（L51）；协议已支持
  thought/tool_call 渲染；无需动 Rust IPC。
- 深链引用点（迁移面）：contacts/{detail-agent,agent-section,agent-detail-drawer,
  endpoint-add-dialog}.tsx、acp-manage/sessions-card.tsx、App.tsx /acp redirect。
- 注册面：menu.def.ts（append-only，现有 7 项 rail，快捷键上限 9）+ App.tsx 路由。
- /agents 已被「智能体管理页」占用；新页路由定为 `/agent`，rail 序插 /chat 后（Cmd+2）。
- main==origin/main @ 2cb41b8a（开工基线）。
- SESSIONS.md 留痕：feat/uix-sidebar 两级树侧栏已并 main@23c9a279（侧栏样式可参照）。

## 拆分与依赖图

ACS1(立) ──合并──> ACS2(破) ──合并──> ACS3(只读评审,可并入收官)

| 任务 | 类型 | 内容 | 分支 |
| --- | --- | --- | --- |
| ACS1 | frontend | 独立 /agent 页全量立起 + redirects 兜底 | feat/acs-standalone |
| ACS2 | frontend | /chat 拆除 agent 形态 + contacts/acp-manage 深链源头改造 | feat/acs-chat-removal |
| ACS3 | code-quality | 只读交叉评审 ACS1+ACS2 diff（只读，不建 worktree） | — |

ACS1 内部提交序（注册类独立小提交纪律）：
1. `chore(gui)`: menu.def.ts rail 注册（append-only 独立小提交）
2. `feat(gui)`: 路由 + redirects + 三区布局壳
3. `feat(gui)`: 侧栏（端点分组会话列表 + 相对时间 + 选中态 + 新建会话）
4. `feat(gui)`: 会话区接线（AgentConversation 复用、连接三态）
5. `test`: 渲染矩阵 + 深链重定向

## 号段与边界

- 分支前缀 feat/acs-*；无迁移号/无共享单根配置文件冲突面（i18n types+locale
  为注册面，ACS1 独占新键命名空间 `agentChat.*`，ACS2 只删不改）。
- 允许触碰：apps/gui/src（acp/、views/chat/、views/agent-chat/ 新建、routes/、
  config/、i18n/、App.tsx）、.orchestrator/本波目录、.agents/collab/SESSIONS.md。
- 禁触：src-tauri、crates/、docs/design（只读）、settings/remote-access（他波）。

## Guesses（过期条件）

- G1: acp-view.tsx 与 acp/ 下旧 ACP 视图可能是死代码（App.tsx 未引用 acp-page）。
  ACS1 执行者先查引用再决定处置；过期条件=ACS1 开工时核实。
- G2: session-sidebar（acp/components）可整体复用为新侧栏底座；过期条件=ACS1
  侧栏开工时读其依赖面，若耦合 acp-view 无法解耦则新写（≤300 行红线内）。

## 进度账本（每事件一行：五问裁决）

- 2026-09-14 16:02 计划 v1 落盘；用户三裁决入账。
- 2026-09-14 16:10 账本+任务书+截图入库 @ ca9ded67（pre-push 门禁 PASS，已推 origin/main）。
- 2026-09-14 16:10 ACS1 派发 session-920a4593-129a-4989-84b6-357abfd36258
  （feat/acs-standalone）；下一步：等汇报，窗口内预写 ACS2 任务书草案。
