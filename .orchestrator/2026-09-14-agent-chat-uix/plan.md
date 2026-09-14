# agent 聊天 UI 波 · 计划 v1（2026-09-14）

## 目标（用户指令逐字）
参考 /Users/imeepos/ext512/ymm-001/deepseek-harness/packages/client，改造 agent 聊天 UI：
区分工作区/工作区下的会话；样式风格尽量复刻或直接复用。

## 事实基线（主控侦察结论）
- 参考实现 = DSH 插件化客户端（ui-* 模块 + slots 插槽 + @deepseek-ai/dsh-client-* 全家桶 +
  CSS Modules + dsw 设计 token）。工作区/会话树 = ui-workspace（WorkspaceBrowser/tree/rows）
  嵌在 ui-sidebar 的 `sidebar.workspaces` 插槽。
- **直接复用不可行**：全家桶运行时依赖与本仓 tailwind v4 + shadcn 风格 + zustand 体系冲突；
  **视觉/交互复刻可行**：CSS Modules 中的几何、token、交互细节可翻译为 tailwind。
- 本仓数据面已就绪：ACP `session/list` 返回 `cwd`；「工作区」= 会话 cwd 目录的自然分组，
  零后端改动（决策：采用 cwd 分组方案，不引入 workspace 实体；触发器=改面最小+数据已就绪）。
- 本仓现状：SessionSidebar 120 行单层列表；无分组/搜索/折叠。

## 任务图
T1（architecture，只读调研）→ T2（frontend，侧栏两级树）∥ T3（frontend，会话区视觉对齐）→ T4（code-quality，只读评审）

- T1 Produces：docs/design/agent-chat-uix-spec.md（token 对照/组件结构/交互清单/数据映射）
- T2 Consumes T1；T3 Consumes T1（消息区部分）
- 预分配：分支前缀 feat/uix-*（t2=feat/uix-sidebar、t3=feat/uix-conversation）；无迁移号需求

## 全局约束（逐份随任务书）
- worktree 协议 + 收尾四步（AGENTS.md）；单文件 ≤300 行、函数 ≤60 行；无 emoji；i18n zh/en 双语；
  注册类改动（menu.def/App.tsx/locales types）独立小提交；测试与实现同提交；
  并行 lead 存在（session-cd1cdce3 权限/服务总控波）：禁触其 scope 与 src-tauri；
  参考实现目录只读。

## 锁定区（本波）
T1 / T2 / T3 / T4 如上。

## 待细化区（触发条件）
- T3 范围收缩/扩张：待 T1 规格文档出 beforeEach 视觉差距清单后细化。
- 工作区实体化（acp-workspaces.json 关联会话）：本波不做；触发器=用户明确要求授权工作区语义。

## 进度账本
见 ledger.md（五问裁决逐事件记行）。
