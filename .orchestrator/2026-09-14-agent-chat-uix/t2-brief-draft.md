# T2 任务书草稿 · frontend · 工作区→会话两级侧栏（待 T1 产出后定稿派发）

## 目标
将 SessionSidebar（120 行单层列表）改造为「工作区→工作区下的会话」两级树，视觉复刻参考实现（uix-spec.md 规格）。

## Consumes
T1 产出的 /Users/imeepos/ext512/p2p/.orchestrator/2026-09-14-agent-chat-uix/uix-spec.md（token 对照/交互清单/数据映射）。

## 实现边界（预置）
- 数据面零后端改动：分组纯函数挂 acp-store（SessionSummary.cwd 分组；无 cwd 会话入「未分组」区）。
- 组件：apps/gui/src/acp/components/session-sidebar.tsx 重写 + 新增分组行组件；i18n zh/en。
- 测试：分组纯函数单测（红绿双向）+ 渲染矩阵（分组/折叠/选中/空态）。
- 砍单候选（以 spec 第 4 节为准）：搜索、排序持久化、滚动条 linger。

## 验收（预置，定稿时逐条可判定化）
- 分组纯函数单测 ≥6 用例（空 cwd/多工作区/单会话/排序稳定性）。
- 渲染矩阵测试 ≥8 断言；jsdom 全绿；tsc/eslint 零错。
- 行数红线：单文件 ≤300 行。
