# T2 任务书 · frontend · 工作区→会话两级侧栏（定稿,派发 session 见账本）

## 目标
将 SessionSidebar（120 行单层列表）改造为「工作区→工作区下的会话」两级树，视觉复刻参考实现（uix-spec.md 规格）。

## Consumes（先读,按序）
1. /Users/imeepos/ext512/p2p/.orchestrator/2026-09-14-agent-chat-uix/uix-spec.md（T1 产出,已验收:token 对照/树规格/数据映射/砍单建议）
2. /Users/imeepos/ext512/p2p/.orchestrator/2026-09-14-agent-chat-uix/before-agent-chat-0914.png（改造前截图,before 证据基准）

## 实现边界（预置）
- 数据面零后端改动：分组纯函数挂 acp-store（SessionSummary.cwd 分组；无 cwd 会话入「未分组」区）。
- 组件：apps/gui/src/acp/components/session-sidebar.tsx 重写 + 新增分组行组件；i18n zh/en。
- 测试：分组纯函数单测（红绿双向）+ 渲染矩阵（分组/折叠/选中/空态）。
- 砍单候选（以 spec 第 4 节为准）：搜索、排序持久化、滚动条 linger。

## 验收（预置，定稿时逐条可判定化）
- 分组纯函数单测 ≥6 用例（空 cwd/多工作区/单会话/排序稳定性）。
- 渲染矩阵测试 ≥8 断言；jsdom 全绿；tsc/eslint 零错。
- 行数红线：单文件 ≤300 行。

## 全局约束（硬性）
worktree 协议 + 收尾四步（分支 feat/uix-sidebar,基于 origin/main）;函数 ≤60 行/文件 ≤300 行;
无 emoji;中文注释独立成行;i18n zh/en 键块（remote-access 无关,本任务只动 acp 侧栏键块）
独立小提交;测试与实现同提交;并行 lead 存在（session-cd1cdce3 禁触其 services scope）。

## 预算与停止条件
实现 1 轮 + 修复 ≤2 轮。uix-spec.md 与实际代码结构冲突导致方案不可行 → BLOCKED 并说明。

## 汇报
session_link_send_parent 回报派发者（显式 id：session-e1cd6aa4-b444-4355-a8a5-cb3daf959e0d）,
状态枚举 DONE / DONE_WITH_CONCERNS / BLOCKED / NEEDS_CONTEXT,附分支名、测试退出码、after 截图路径。
