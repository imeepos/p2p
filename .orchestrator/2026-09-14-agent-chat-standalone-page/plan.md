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
- 2026-09-14 16:50 ACS1 执行留痕（session-920a4593，分支 feat/acs-standalone
  @ 7 个提交，rebase 后基线 origin/main@9bb45b6f）：
  - Guess 验证：G1 成立一半——acp-page.tsx 路由挂载点仅剩自身测试引用，
    但 AcpView 被 8 个测试文件与 palette 注册面引用，非死代码，本轮不动；
    G2 不成立——session-sidebar 只依赖 acp-store/workspace-model，与
    acp-view 无耦合（可复用），但其树按 workspace 分组且无端点维度、
    sessions 仅属当前连接，故新写端点-会话两级侧栏（agentChat 侧栏四件套）。
  - 数据面缺口（如实上报）：store 无 per-session 时刻，会话行相对时间只在
    「当前端点+当前会话」取 lastInteractionByEndpoint（语义即当前会话最后
    交互），其余行不显时间不冒充；ACS2+ 如需每会话历史时间需扩 store（本波
    禁触协议面）。
  - 门禁：make check-fast exit 0（内含 gui-check：lint+build+247 文件/1529
    测试绿）；make gui-check 独立复跑 exit 0；聚焦矩阵 11 例 + redirects
    全应用挂载 15 例绿。
  - 走查证据：evidence/acs1-agent-{empty,disconnected,connected}.png
    （VITE_MOCK_IPC=1 dev 5199，rail 第二项 /agent、侧栏端点清单含本机徽标、
    选中态/离线提示/在线三区+新建会话按钮均实证；会话行相对时间的走查需真实
    mock 连接，以矩阵测试 agent-session-time-s-001 断言为证）。
  - 交接注意：/acp 存量守卫 src/test/app-redirects.test.tsx 已随改指更新并补
    agent 深链两例；palette-nav 守卫基线 16->17 项；/chat 页内零改动。
- 2026-09-14 16:55 主干验收合并 ACS1：ff-only @ 5d2fadd8 已推 origin/main；
  主干 check-fast 复跑 exit 0（panic-hygiene/protocol-registry/cli-parity/
  ai-docs-sync 全 PASS）。
- 2026-09-14 16:55 Ruling: 偏差①（会话行相对时间仅当前会话显示）接受现状——
  SessionSummary 协议无时间字段（protocol.ts L58-62 仅 sessionId/title/cwd），
  子会话不冒充端点时刻是正确处置；per-session 时刻需客户端交互日志（纯前端
  可做但历史会话冷启动无数据），入候选池不进 ACS2。
- 2026-09-14 16:55 Guess 裁定：G1 半成立（AcpView 非死代码，8 测试+palette
  引用，保持不动）；G2 不成立（新写端点-会话两级侧栏正确）。
- 2026-09-14 16:56 收尾启动：worktree remove（主控执行成功）。
- 2026-09-14 17:00 竞态事件与裁定：子会话在汇报后按 AGENTS.md 收尾四步自行
  续做——补推 self-evolving 复盘提交 170273d6（docs-only）→ 主树 ff-only 合并
  → push origin main → 分支/远端双删。与主控清理指令竞态（branch -d 曾短暂
  失败，后自愈）。内容无损，最终态 main==origin/main@170273d6、worktree/分支
  全清。Ruling: 接受现状；流程缺口在主控任务书——只写「不自行合并主干」未禁
  收尾②③④，ACS2 起任务书显式改为「子会话仅做①push 分支，②③④一律主控执行」。
- 2026-09-14 17:01 ACS1 事实闭环：验收 PASS（7+1 提交、门禁双绿、三态截图、
  偏差①裁定入候选池）；主干 @ 170273d6；ACS1 会话 920a4593 暂不归档
  （留修复循环通道，ACS3 评审结论后处置）。下一步：ACS2 定稿派发 +
  ACS3 只读评审（ACS1 diff，subagent 换模型交叉评审）并行。
- 2026-09-14 17:05 ACS1 结单补遗收讫（session-920a4593）：与主控观察一致，
  无新事实；回执告知 ACS2 已派新会话（一个任务一个会话，不续用）、本会话
  保持存活作 ACS1 修复循环通道。ACS2 在飞 session-b2d05d3f；
  ACS3 评审在飞 subagent 634ae79e（deepseek-v4-flash，只读）。
- 2026-09-14 18:19 评审子代理换模型（用户指令「那个模型应该限流了」）：
  634ae79e（deepseek-v4-flash）运行 1h12m 零产出，判限流卡死，interrupt
  终止（无残留产物）；volces 对本会话不可用，bigmodel 仅 GLM-5.3-Flash
  （与实现者同族不合规）；重派 a9264b34（minimax-cn/MiniMax-M3，只读，
  同一评审 prompt）。同期核活：ACS2 session-b2d05d3f 健康在飞
  （worktree @ 0883e722，2 分钟前有提交），不干预。
