# 任务 AF1：新建会话按钮反馈闭环（点击-loading-成功微提示-错误轻提示）

## 目标
本机 agent 聊天页「新建会话」从点击起有即时 pending 微反馈，期间禁止重复触发；
成功弹「会话已创建」微提示，失败弹错误轻提示（同一失败只弹一次）。

## 背景（指针，不是内容）
- 波次计划：`.orchestrator/2026-09-14-agent-chat-feedback/plan.md`（根因链：ACP initialize
  最坏 120s 阻塞，按钮无 pending → 用户连点，提交 1709ddd0 / 66c5a147）。
- `AGENTS.md` —— 硬性规范（行数红线/提交纪律/worktree 协议），开工先读。
- 基建已存在，直接复用，禁重复造轮子：
  - `apps/gui/src/components/feedback/async-button.tsx` —— AsyncButton：重入守卫
    （非 idle 直接 return）、loading spinner + aria-busy + disabled、成功/失败图标、
    loadingLabel、iconOnly。零改动使用；确需小改先在汇报里说明理由。
  - `apps/gui/src/components/feedback/toast.ts` —— toastSuccess / toastError（带去重与
    复制详情），错误 toast 用 `context` 传 `acp.newSession`。
- 现状入口（两处都接）：
  - `apps/gui/src/views/chat/agent-conversation.tsx:190`（data-testid="agent-new-session"）
  - `apps/gui/src/acp/components/session-sidebar.tsx:90`
- store：`apps/gui/src/acp/acp-store.ts` 的 `newSession`（runNewSession 从别处 import，自行定位）。
- 存量失败 toast 断言：`apps/gui/src/acp/acp-flow-failures.test.tsx:161`
  「online 下新建会话失败弹 toast（页面可见）」——先找出现在这个 toast 从哪弹
  （store 内 or 视图层），复用其 i18n key 与路径。

## 验收标准（全部满足才可报 DONE）
- [ ] store 单飞：runNewSession in-flight 期间再次调用返回同一 promise（不并发第二次 IPC）；
      新增 `newSessionPending` 布尔，开始置 true、结束（成功或失败）置 false。先写失败测试再实现（红绿双向）。
- [ ] 两个新建会话入口都换 AsyncButton：点击即时 spinner + disabled；store pending 为 true 时
      两处同时 disabled（跨入口防重入）。渲染矩阵测试断言：pending 态两按钮禁用、
      结束后恢复；连点只触发一次 store 动作。
- [ ] 成功：toastSuccess(t("chat.feedback.sessionCreated"))；失败：错误 toast 只出现一次
      （与存量失败 toast 不得双弹——定位后二选一：留存量、onError 不再弹，或把存量迁移到
      onError 统一路径；测试断言 toast 文案计数 = 1）。
- [ ] i18n：zh/en/types.ts 三处同步；先 grep locales 复用既有 key（如已有「新建会话失败」），
      缺才新增 `chat.feedback.sessionCreated` 等键；i18n 改动为独立小提交（仓库注册类纪律）。
- [ ] 存量相关测试全部保持绿：acp-flow-failures / acp-view-interactions / session-sidebar.test 等。
- [ ] 自验命令真实退出码 0 并贴输出摘要：`cd apps/gui && pnpm vitest run --no-file-parallelism <聚焦文件>`、
      `pnpm typecheck`、`pnpm lint`（限改动文件范围可接受）。
- [ ] 单文件 ≤300 行、单函数 ≤60 行；禁止 emoji；console 零新增报错（测试输出佐证）。

## 边界（明确不做）
- 独占文件之外禁改：`agent-conversation.tsx`、`session-sidebar.tsx`（及其 test）、
  `acp-store.ts`、`chat.feedback.session*` i18n 键块、`async-button.tsx`（尽量零改动）。
  **并行会话 AF2 正在改 views/chat 与 acp/components 的其余组件——见一个改一个，绝不顺手。**
- 不动 120s 超时包络（66c5a147 已做）；不做服务端防重；不做会话分组/侧栏结构重构（那是
  另一波 uix 的规划）；不碰 src-tauri / settings / remote-access。
- 不引入新依赖；不改 toast.ts / confirm-provider 公共行为。

## 接口契约
- Consumes：AsyncButton（主干既有，签名见源码）、toastSuccess/toastError、acp-store.newSession。
- Produces：acp-store `newSessionPending`（AF2 与后续组件可订阅）；无新组件。

## 预算与停止条件
- 预算：≤5 个检查点；修复轮次上限 2。
- 立即报 BLOCKED：worktree 建不出 / origin/main 落后需大量解冲突 / 与 AF2 文件清单出现重叠。
- 立即报 NEEDS_CONTEXT：存量失败 toast 与新错误 toast 无法做到单弹且需改 toast.ts 公共行为。

## 工作区与汇报
- `git fetch origin` 后 `git worktree add ../.worktrees/acf-af1 -b feat/acf-af1-session-feedback origin/main`
  （主树在 /Users/imeepos/ext512/p2p；worktree 目录 .worktrees/acf-af1）。禁止直接改主干、禁止合并、禁止推送。
- 检查点节奏：每完成一个可提交单元 → commit（message 按 `type(scope): subject`，正文写 why）→
  进度一行 append 到 `.orchestrator/2026-09-14-agent-chat-feedback/af1-progress.md`（早落盘小步写，
  首个检查点 30 分钟内必须落盘）。
- 汇报用 `session_link_send_parent` 回报派发者；解析异常或失败时改用
  `session_link_send` 定向投递 `session-a8e83de1-d058-443a-82e9-21cbe2556d60`，不重发试错。
- 追加消息契约：`[补充]` 并入当前工作；`[纠偏]` 当前工具调用完成后停下先回执；`[收尾]`
  落盘当前检查点即报 DONE_WITH_CONCERNS 停止；不可逆动作只在检查点边界做且先请示。
- 状态只用：DONE / DONE_WITH_CONCERNS / BLOCKED / NEEDS_CONTEXT。DONE 必附：变更文件清单、
  逐条验收证据（命令+退出码）、顾虑、结构化复盘（完成度/新事实/缺陷隐患/过度设计候选/
  下游影响/预估偏差/遗留处置/下一步建议/大白话总结，≤10 行）。
- 你看不到派发者的会话历史：以上即全部上下文，疑问开工前一次问清。
