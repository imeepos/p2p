# 任务 AF2：agent 聊天页全按钮反馈走查接线（点击-loading-成功微提示-错误轻提示）

## 目标
agent 聊天页（views/chat 会话区 + acp/components）内所有**异步**交互按钮具备统一反馈：
点击即时 pending 微反馈、成功微提示、错误轻提示；同步 toggle 不弹 toast 防提示洪水。

## 背景（指针，不是内容）
- 波次计划：`.orchestrator/2026-09-14-agent-chat-feedback/plan.md`；`AGENTS.md` 开工先读。
- 基建主干既有，直接复用，禁重复造轮子：
  - `apps/gui/src/components/feedback/async-button.tsx` —— AsyncButton（重入守卫/loading
    spinner/aria-busy/成功失败图标/loadingLabel/iconOnly）；
  - `apps/gui/src/components/feedback/toast.ts` —— toastSuccess / toastError（去重 +
    复制详情；错误必传 `context`，值用 IPC 通道或动作名）；
  - `apps/gui/src/components/feedback/copy-button.tsx` —— 已达标的参照样板，先读它再动手。
- 走查域（允许改动路径）：
  - `apps/gui/src/views/chat/`（chat-empty-state / group-pending-panel / chat-page /
    friend|group|a2a-conversation 等）
  - `apps/gui/src/acp/components/`（connection-card / local-acp-card / prompt-composer /
    permission-notice-bridge / share-create-dialog / share-create-fields / share-join-card /
    share-manage-card / share-send-targets / transcript-tools / usage-bar 等）
- 枚举方法：在上述两目录 grep `<Button`、`onClick`，逐个分类，结果记入进度文件（见检查点 1）。

## 分类判据（写进走查清单，逐按钮给一行裁决）
- 异步动作（IPC / store promise / 网络）→ AsyncButton 接线 + 成功/错误 toast。
- 纯本地同步 toggle（折叠/展开/切换显隐）→ 不弹 toast；确认有可见按压反馈即可，记录「无需接线」。
- 已有完整反馈面 → 审计记录「已达标」，禁止重造：composer 发送/停止（use-retry-send、
  send-notify 已有失败链路）、copy-button、destructive-confirm 确认弹框流程。
- 确认弹框内执行异步的确认按钮 → 弹框流程不动，按钮本体换 AsyncButton。
- 组件若同时挂载在 agent 聊天页之外且改动会影响他页 UX → 不改，记录到汇报顾虑节。

## 验收标准（全部满足才可报 DONE）
- [ ] 走查清单落盘：每个交互控件一行（文件:行 / 分类 / 裁决「接线|无需|已达标」），无遗漏。
- [ ] 判为「接线」的按钮全部完成：pending 可见（spinner+disabled）、成功微提示、
      错误轻提示；错误 toast 带 context；同一失败不与既有 toast 双弹。
- [ ] 每个新接线按钮至少 2 条测试断言（红绿双向）：pending 态可见/禁用；失败路径错误 toast
      出现且文案正确。测试放对应组件的 test 文件或新建，跟随组件域。
- [ ] 存量测试全绿（含 acp-view / chat-* / share-* 既有矩阵）；`pnpm typecheck`、`pnpm lint` 退出码 0。
- [ ] i18n：zh/en/types.ts 三处同步；新文案仅放 `chat.feedback.actions.*` 子键块
      （`chat.feedback.session*` 归并行会话 AF1，禁用其键名）；能复用 common 既有 key 就复用；
      i18n 改动独立小提交。
- [ ] 单文件 ≤300 行、单函数 ≤60 行；禁止 emoji；console 零新增报错。

## 边界（明确不做）
- **禁改**：`agent-conversation.tsx`、`session-sidebar.tsx`（及其 test）、`acp-store.ts`、
  `chat.feedback.session*` 键块——并行会话 AF1 正在改，见一个绕一个。
- 不碰 `components/feedback/*`（基建零改动）、`components/chat/*`（composer 发送链路只审计）、
  src-tauri / settings / remote-access / transcript-model / 协议层。
- 不引入新依赖；不重构组件结构；不做视觉改版（uix 波的事）——只加反馈行为。
- scope 外发现的问题记入汇报，不顺手修。

## 接口契约
- Consumes：AsyncButton / toastSuccess / toastError（主干既有，签名见源码）。
- Produces：无新契约；i18n `chat.feedback.actions.*` 键块归你独占。

## 预算与停止条件
- 预算：≤5 个检查点；修复轮次上限 2。
- 立即报 BLOCKED：worktree 建不出 / 与 AF1 文件清单出现重叠 / origin/main 落后需大规模解冲突。
- 立即报 NEEDS_CONTEXT：某控件异步与否无法判定且影响接线决策（列出控件与疑点）。

## 工作区与汇报
- `git fetch origin` 后 `git worktree add ../.worktrees/acf-af2 -b feat/acf-af2-button-walkthrough origin/main`
  （主树 /Users/imeepos/ext512/p2p；worktree 目录 .worktrees/acf-af2）。禁止直接改主干、禁止合并、禁止推送。
- 检查点节奏：①走查清单落盘（先做，30 分钟内）→ ②views/chat 接线批 → ③acp/components 接线批
  → ④i18n 键块（提交前 `git fetch origin`，若 AF1 已并主干先 rebase 再提交 i18n）→ ⑤自验汇报。
  每检查点 commit + 进度一行 append 到
  `.orchestrator/2026-09-14-agent-chat-feedback/af2-progress.md`。
- 汇报用 `session_link_send_parent` 回报派发者；解析异常或失败时改用
  `session_link_send` 定向投递 `session-a8e83de1-d058-443a-82e9-21cbe2556d60`，不重发试错。
- 追加消息契约：`[补充]` 并入当前工作；`[纠偏]` 当前工具调用完成后停下先回执；`[收尾]`
  落盘当前检查点即报 DONE_WITH_CONCERNS 停止；不可逆动作只在检查点边界做且先请示。
- 状态只用：DONE / DONE_WITH_CONCERNS / BLOCKED / NEEDS_CONTEXT。DONE 必附：变更文件清单、
  走查清单全文、逐条验收证据（命令+退出码）、顾虑、结构化复盘（完成度/新事实/缺陷隐患/
  过度设计候选/下游影响/预估偏差/遗留处置/下一步建议/大白话总结，≤10 行）。
- 你看不到派发者的会话历史：以上即全部上下文，疑问开工前一次问清。
