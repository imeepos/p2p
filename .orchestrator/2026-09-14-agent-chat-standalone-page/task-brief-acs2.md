# ACS2 任务书：聊天页拆除 agent 形态（破）

类型：frontend ｜ 分支 feat/acs-chat-removal ｜ worktree：.worktrees/acs-chat-removal
派发者显式 id：session-3aa89cd2-c9d9-40f8-b20b-d33412bdf5d5
波次账本：.orchestrator/2026-09-14-agent-chat-standalone-page/plan.md
基线：origin/main @ 170273d6（ACS1 已并，/agent 页与 redirects 兜底在位）

## 目标（一句话）

agent 会话唯一入口收敛到 `/agent`：/chat 移除 agent 形态与会话列表条目，
contacts 与 acp-manage 深链源头改指新页，i18n 死键清理；旧链接兼容由 ACS1
已落位的 routes/agent-redirect.ts 兜底，本任务不删兜底。

## ACS1 交接事实（已核，直接引用）

- /agent 页：apps/gui/src/views/agent-chat/ 四件套 + routes/agent-redirect.ts
  （纯函数 + 单测 5 例）+ chat-route 命中才 Navigate（/chat 页内零改动）。
- 存量守卫已随 ACS1 更新：src/test/app-redirects.test.tsx（/acp 落点改指
  /agent + 补 agent 深链两例）、palette-nav 守卫基线 17 项。
- 会话行相对时间缺口已裁定入候选池（SessionSummary 无时间字段），ACS2 不处理。

## 验收标准（可机械核验）

1. /chat 页与 use-conversation-entries 无 agent 分支：grep
   `AgentConversation|agentParam|kindParam.*agent` apps/gui/src/views/chat/
   零实现命中（测试内断言行为变化的除外）；聊天页渲染矩阵测试更新后绿。
2. 深链源头改造：views/contacts/{detail-agent,agent-section,agent-detail-drawer,
   endpoint-add-dialog}.tsx、views/acp-manage/sessions-card.tsx 内 `chat?agent=`
   /`kind=agent` 源头零命中，改指 `/agent?endpoint=...`（encodeURIComponent
   一致）；grep 全 src 仅 routes/redirects 及其测试保留兼容层。
3. agent-conversation.tsx、agent-conversation-inject.ts、agent-permission-banner*
   拆除后无引用即退役（git rm）；agent-conversation-inject 的 VITE_MOCK_IPC
   注入入口确认新页已带等价物（ACS1 渲染矩阵已用 mock，核对后处置）。
4. i18n 死键清理：删除的 chat.agentPane.* 键同步 types + zh/en locale；
   注册类改动（i18n types+locale）压独立小提交。
5. 全应用挂载测试（app-redirects/palette-nav/chat-page 相关）更新后绿；
   聚焦测试 + typecheck + lint 退出码 0；console 零新增报错。
6. 门禁：make check-fast exit 0（贴退出码）。

## 输入（路径，自己读）

- apps/gui/src/views/chat/chat-page.tsx、use-conversation-entries.ts（拆除主体）
- apps/gui/src/views/agent-chat/（新页，勿动其内部行为）
- apps/gui/src/routes/agent-redirect.ts + redirects.tsx（兜底在位，不删）
- views/contacts/ 四文件、views/acp-manage/sessions-card.tsx（源头改造）
- .orchestrator/2026-09-14-agent-chat-standalone-page/plan.md（ACS1 交接节）

## 边界（明确不做）

- 不动 /agent 新页内部行为（测试 import 路径调整等 minimal 改动需在汇报单列）。
- 不删 routes/agent-redirect.ts 兼容兜底；不做新功能；纯迁移+拆除。
- 不处理会话相对时间缺口（候选池）；不动 Rust/IPC。
- 单文件 ≤300 行、单函数 ≤60 行；禁止 emoji 图标；提交 type(gui): subject。

## 产出物

- 代码拆除 + 深链源头改造 + i18n 清理 + 测试更新；每完成一小步即 commit。
- 汇报：session_link_send_parent 回派发者（解析异常定向投
  session-3aa89cd2-c9d9-40f8-b20b-d33412bdf5d5）；状态枚举 + 行为证据
  （分支名、commit 列表、grep 零命中输出、测试退出码）+ 结构化复盘。

## 流程（AGENTS.md 硬协议 + 主控修正）

- 开工：`git fetch origin && git worktree add .worktrees/acs-chat-removal -b
  feat/acs-chat-removal origin/main`；SESSIONS.md append 认领行（会话 id /
  scope=chat agent 形态拆除 / 分支 / 派发者）。
- 早落盘小步写；合并前 `git rebase origin/main`（禁 git merge main），跑门禁后回报。
- **收尾边界（主控修正，覆盖 AGENTS.md 收尾四步的执行人）**：子会话仅做
  ①`git push origin <分支>`；②主树合并、③worktree remove、④分支删除
  一律由主控执行——子会话禁止在主树执行 merge/push main/分支删除，汇报后即停。
- 轻量自验=聚焦测试 + typecheck + lint；全量门禁留给主控。

## 预算与停止条件

- 修复轮次预算 ≤2；总工作量 ≤半天。
- 立即 BLOCKED：拆除面与新页存在未预见的共享状态耦合、
  深链改造会破坏 contacts 现有行为且无替代路径。
