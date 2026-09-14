# ACS1 任务书：独立 Agent 会话页（立）

类型：frontend ｜ 分支 feat/acs-standalone ｜ worktree：.worktrees/acs-standalone
派发者显式 id：session-3aa89cd2-c9d9-40f8-b20b-d33412bdf5d5
波次账本：.orchestrator/2026-09-14-agent-chat-standalone-page/plan.md

## 目标（一句话）

新建 rail 一级入口 `/agent` 独立 Agent 会话页：三区布局（左侧端点/会话侧栏 +
中部对话流 + 底部 composer），覆盖所有 ACP 端点，旧 `/chat?agent=` 深链重定向兜底；
本波**不动** /chat（拆除属 ACS2）。

## 视觉参照（必须看）

`.orchestrator/2026-09-14-agent-chat-standalone-page/reference/dsh-agent-ui.png`
—— 用户期望的布局基准：左侧会话列表（条目右侧相对时间）、中部对话流
（头部会话名）、底部输入框。像素级还原不做，结构与密度对齐；复用本项目
既有设计语言（shadcn/tailwind，hairline 边框规范见 uix-spec §3）。

## 验收标准（可机械核验）

1. `menu.def.ts` rail 注册为独立小提交（append-only 注释注明 2026-09-14 用户拍板）；
   rail 序插 /chat 后；快捷键序顺延不越 9。
2. `/agent` 路由可达；未选端点显空态；选端点后为「侧栏 + 会话头 + Transcript +
   PromptComposer」三区；narrow（<768px）单栏互斥与 /chat 同规则。
3. 侧栏：所有 saved endpoints（本机 + 远端）可达，选中态高亮；端点下会话列表
   （数据面 acp-store.sessions，禁新增 IPC）含相对时间；新建会话按钮复用
   AF1 反馈闭环（AsyncButton + newSessionPending 单飞 + toast）。
4. redirects：`/chat?agent=X` → `/agent?endpoint=X`；`/chat?kind=agent` →
   `/agent`；`/acp` redirect 目标改 `/agent`；redirects.tsx 单元测试覆盖三条。
5. `/chat` 路由行为**不变**（agent 条目仍显示——拆除是 ACS2 的活，禁越界）。
6. 渲染矩阵测试：新页覆盖 connecting / connected / disconnected 三态 +
   空态 + 会话选中态；聚焦测试、typecheck、lint 退出码 0；console 零新增报错。
7. 门禁：项目等效 check-fast + gui-check 全绿（退出码贴汇报）。

## 输入（路径，自己读）

- apps/gui/src/views/chat/agent-conversation.tsx（连接三态/引导卡/权限条，复用主体）
- apps/gui/src/acp/components/session-sidebar.tsx + acp/acp-view.tsx（G2：先查引用，
  可解耦则复用为侧栏底座，耦合死则新写侧栏组件）
- apps/gui/src/acp/acp-store.ts（sessions: SessionSummary[] L51、focusedEndpoint、
  connect/newSession 动作）
- apps/gui/src/config/menu.def.ts、App.tsx 路由表、routes/redirects.tsx
- apps/gui/src/views/chat/chat-page.tsx（narrow 互斥规则参照）
- .agents/collab/SESSIONS.md 尾部（feat/uix-sidebar 两级树侧栏已并 main，
  样式可参照其模式）

## 边界（明确不做）

- 不动 /chat 页内 agent 形态与 use-conversation-entries（ACS2 专属）。
- 不改 contacts/、acp-manage/ 的深链源头（ACS2 专属；redirects 已兜底）。
- 不做任务清单卡片、双 tab、附件、模型选择器、会话搜索、视觉大改版。
- 不动 Rust/IPC/store 协议面；发现缺口即 BLOCKED 上报，不自行扩范围。
- 单文件 ≤300 行、单函数 ≤60 行；禁止 emoji 图标。

## 产出物

- 代码：apps/gui/src/views/agent-chat/（新目录）+ 上述注册面改动；
  i18n 新键全部收 `agentChat.*` 命名空间（types + zh/en locale 同步）。
- 测试：新页渲染矩阵 + redirects 三条用例。
- 汇报：session_link_send_parent 回派发者；状态枚举 DONE / DONE_WITH_CONCERNS /
  BLOCKED / NEEDS_CONTEXT + 行为证据（分支名、commit 列表、测试退出码、
  走查截图路径）+ 结构化复盘（偏差/ Guesses 验证结论/交接注意）。
- 提交纪律：type(gui): subject；正文写机理；注册类（menu.def/i18n types）独立小提交；
  每完成一小步即 commit 落盘。

## 流程（AGENTS.md 硬协议）

- 开工先 `git fetch origin && git worktree add .worktrees/acs-standalone -b
  feat/acs-standalone origin/main`；并在 .agents/collab/SESSIONS.md append 认领行
  （会话 id / scope=agent 独立会话页 / 分支 / 派发者）。
- 早落盘小步写：先骨架 commit，再逐块推进（派发后验活机制）。
- 合并前 `git rebase origin/main`（禁 git merge main），跑门禁后回报；**不自行合并
  主干**（主干合并由主控执行）。
- 轻量自验=聚焦测试 + typecheck + lint；全量门禁留给主控。

## 预算与停止条件

- 修复轮次预算 ≤3（打回附具体差距）；总工作量 ≤1 个工作日。
- 立即 BLOCKED：acp-store 数据面不足以支撑侧栏（需动 IPC/协议）、
  session-sidebar 解耦会破坏既有 acp-view 行为且无替代、i18n 类型系统阻塞。
