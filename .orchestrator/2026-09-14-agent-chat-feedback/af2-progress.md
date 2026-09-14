# AF2 进度账本（feat/acf-af2-button-walkthrough @ origin/main effd6e4f）

## 检查点日志
- ① 走查清单落盘（2026-09-14 23:33 -07:00）：views/chat + acp/components 全量枚举完毕，控件 51 行见下表；接线判定 5 按钮 4 文件（均在 acp/components，views/chat 全域为无需/已达标/AF1 范围）。
- ② views/chat 接线批：无接线项（走查结论：全部为同步导航/同步 toggle/已达标/AF1 禁改），无代码变更，不占提交。
- ③ acp/components 接线批（提交 c2308575）：5 按钮接线完成，测试先红（6 红断言）后绿（11/11）；每按钮 ≥2 断言（pending disabled/aria-busy + 失败 toast 文案）。
- ④ i18n 键块（提交 9aa84cc6，先于接线提交落地）：chat.feedback.actions.{refreshed,refreshFailed,revoked} zh/en 两处同步；types.ts 为 zh-CN 结构自动推导免改；提交前已 fetch 确认 AF1 未并主干（origin/main 仍 effd6e4f）。
- ⑤ 自验（2026-09-14 23:53 -07:00）：聚焦矩阵 src/acp + views/chat + components/chat + components/feedback 共 79 文件 500 测试全绿；pnpm typecheck 退出码 0；pnpm lint 退出码 0；改动文件 emoji 零命中；单文件最大 263 行（share-manage-card.tsx）。

## 状态：DONE_WITH_CONCERNS（待主控验收）
顾虑三条见汇报：① local-acp-card 刷新因 listWorkspaces 吞错契约无法上错误 toast；② connection-card connect/disconnect 为 store void 契约判已达标（状态驱动反馈），如需 toast 需 store 契约变更（acp-store 归 AF1）；③ AF1 文件（agent-conversation/session-sidebar/acp-store）按边界未动，其内新建会话等按钮由 AF1 负责。

## 走查清单（文件:行 / 控件 / 分类 / 裁决）

### views/chat/
1. chat-page.tsx:188 `chat-back` | 同步路由返回（setSearchParams）| 无需接线
2. chat-page.tsx:171→conversation-list.tsx:92 会话列表重试 | 异步重载（components/chat 禁改）| 已达标（本体已是 AsyncButton）
3. chat-empty-state.tsx:54 `chat-empty-add-friend` | 同步导航 | 无需接线
4. chat-empty-state.tsx:63 `chat-empty-go-contacts` | 同步导航 | 无需接线
5. friend-conversation.tsx:83 `chat-share-acp` | 同步开弹框 | 无需接线
6. friend-conversation.tsx:98 MessageList onLoadOlder | 异步历史加载（components/chat 禁改）| 已达标（loadingOlder 态呈现）
7. friend-conversation.tsx:99 onCancelPending | composer/send-notify 链路 | 已达标（任务书点名链路）
8. friend-conversation.tsx:101 onRetry | use-retry-send 链路 | 已达标（任务书点名链路）
9. friend-conversation.tsx:106 Composer 发送 | composer 发送链路 | 已达标（禁重造）
10. group-pending-panel.tsx:36 `group-pending-timeout-copy` CopyButton | 剪贴板 | 已达标（参照样板本体）
11. inactive-groups-toggle.tsx:19 Switch 显隐 | 纯本地同步 toggle（localStorage 写穿）| 无需接线
12. a2a-conversation.tsx:147 Composer（transport 注入）| composer 链路，handleSend throw 由 composer 统一 toast | 已达标
13. agent-conversation.tsx:69 `agent-console-guide-logs` Link | 同步导航 | 无需接线（AF1 文件禁改，审计）
14. agent-conversation.tsx:94 `agent-connect-error-copy` CopyButton | 剪贴板 | 已达标（禁改审计）
15. agent-conversation.tsx:102 `agent-edit-link` Link | 同步导航 | 无需接线（AF1 禁改审计）
16. agent-conversation.tsx:133 `agent-permission-banner-action` Link | 同步导航 | 无需接线（AF1 禁改审计）
17. agent-conversation.tsx:190 `agent-new-session` | 异步 newSession | AF1 接线范围（本会话禁改，见汇报顾虑）
18. agent-conversation.tsx:236 `agent-connect` | 异步拨号（store void 契约）| AF1 范围禁改；现状 connecting 有专示（agent-connecting spinner）

### acp/components/
19. connection-card.tsx:126 `acp-retry-connect` | 异步 connect（store 契约 `connect: () => void`）| 已达标（offline 面板即失败反馈：原因行+重试入口；phase 徽章全程可见，无可 await 的 promise）
20. connection-card.tsx:160 saved chip 回填 | 同步 setDraft | 无需接线
21. connection-card.tsx:167 saved chip 移除 | 确认弹框+同步 removeSaved | 已达标（destructive-confirm 流程）
22. connection-card.tsx:217 `acp-disconnect` | 异步断开（void 契约）| 已达标（phase 徽章状态驱动，无成功/失败事件可挂 toast）
23. connection-card.tsx:221 `acp-connect` | 异步拨号（void 契约）| 已达标（connecting 相位徽章 + Notices 错误行 + offline 面板）
24. connection-card.tsx:225 `acp-save-endpoint` | 同步 saveDraft（本地持久化）| 无需接线
25. connection-directory.tsx:63 行回填 | 同步 setDraft | 无需接线
26. connection-directory.tsx:88 `acp-directory-remove-*` | 确认弹框+同步 removeDirectoryEntry | 已达标（destructive-confirm）
27. connection-directory.tsx:157 `acp-directory-add` | 同步 addManualPeer + role=alert 行内校验 | 无需接线
28. local-acp-card.tsx:54 `acp-local-manage` | 同步 hash 导航 | 无需接线
29. local-acp-card.tsx:68 `acp-local-refresh` | 异步 listWorkspaces 重取，点击零反馈、失败静默置空 | 接线（AsyncButton iconOnly + 成功/错误 toast）
30. local-acp-card.tsx:77 `acp-local-share` | 同步开弹框 | 无需接线
31. local-acp-card.tsx:113 `acp-local-share-<id>` 每行 | 同步开弹框 | 无需接线
32. prompt-composer.tsx:51 `acp-composer-send` | composer 发送链路 | 已达标（任务书点名）
33. prompt-composer.tsx:59 `acp-composer-stop` | composer 停止链路 | 已达标（任务书点名）
34. session-sidebar.tsx:47 行 resume | 异步 resumeSession | AF1 范围（禁改，审计）
35. session-sidebar.tsx:58 `acp-session-close-*` | 确认弹框+异步 closeSession | AF1 范围（禁改，审计）
36. session-sidebar.tsx:86 `acp-session-new` | 异步 newSession | AF1 接线范围（禁改）
37. share-create-dialog.tsx:232 `acp-share-send` | 异步 sendToChat：有错误 toast、缺 pending | 接线（AsyncButton；成功=关弹框+调用方 toast，失败 toast 已带 context=acp-share-send 不双弹）
38. share-create-dialog.tsx:236 `acp-share-create` | 异步 createShare | 已达标（creating 标签+disabled、成功行内显链、失败 toast 带 context）
39. share-create-dialog.tsx:222 `acp-share-copy` CopyButton | 剪贴板 | 已达标（样板）
40. share-join-card.tsx:57 `acp-share-join-action` | 异步 join 状态机 | 已达标（joining disabled+标签、joined/denied/invalid 行内态）
41. share-manage-card.tsx:170 `acp-share-manage-refresh` | 异步 listShares 重取，点击零反馈 | 接线（AsyncButton iconOnly + toast；保留行内 loadFailed 面板）
42. share-manage-card.tsx:179 `acp-share-manage-create` | 同步开弹框 | 无需接线
43. share-manage-card.tsx:199 `acp-share-manage-reload`（错误行重试）| 异步 listShares，缺 pending | 接线（同 41 动作复用）
44. share-manage-card.tsx:105 `acp-share-revoke-*` | 确认弹框+异步 revokeShare：失败 toast 已有，缺 pending 与成功提示 | 接线（按钮 spinner+disabled；成功 toast「已撤销」；弹框流程不动）
45. share-send-targets.tsx:93 `acp-share-send-targets` | 异步逐个发送 | 已达标（sending 标签+disabled、行内 summary 全成/部分失败）
46. share-send-targets.tsx:77 target checkbox | 本地多选同步 | 无需接线
47. transcript.tsx:42 `acp-thought-toggle-*` | 同步折叠（aria-expanded）| 无需接线
48. transcript.tsx:90 `acp-turn-retry-*` | 异步 sendPrompt 重试 | 已达标（runActive 禁用+进行时条；失败=红色 stopReason 徽章常驻）
49. transcript-tools.tsx:49,:73 折叠 toggle | 同步（aria-expanded/controls）| 无需接线
50. permission-notice-bridge.tsx | 无按钮（toast 桥组件）| 无交互控件
51. usage-bar.tsx / share-create-fields.tsx | 无按钮 | 无交互控件

接线合计：5 按钮 / 4 文件（29、37、41、43、44），全部在 acp/components。

### 追加域（[补充] 2026-09-14 23:56，components/chat 两文件，仍守判据；签名证据版见下）
52. conversation-row.tsx:88 行按钮 onSelect/onContextMenu | 同步选择+右键坐标上抛（`onSelect: (entry) => void`，菜单上抛 MouseEvent）| 无需接线（sendState 图标/未读角标/静音铃均为可视状态反馈面）
53. conversation-context-menu.tsx:53 `conversation-menu-pin` | `togglePinned: (key: ConversationKey) => void`（conversation-prefs-store.ts:32，localStorage 写穿）| 无需接线（置顶即时可见：行上移）
54. conversation-context-menu.tsx:59 `conversation-menu-unread` | `setManualUnread: (key, value: boolean) => void`（:34）+ `markPeerRead: (peer: string) => void`（chat-store.ts:57）+ `markGroupRead: (groupId: string) => void`（group-store.ts:54）+ `markEndpointRead: (endpointId: string) => void`（acp-store.ts:71）| 全部同步 void → 无需接线（红点即时消长）
55. conversation-context-menu.tsx:67 `conversation-menu-mute` | `toggleMuted: (key: ConversationKey) => void`（:33）| 无需接线（铃图标即时显隐）
56. conversation-context-menu.tsx:70 `conversation-menu-window` | `openConversationWindow: (entry) => Promise<void>`（异步）| 已达标不重造：失败 console.error + toastError 带 context=chat.openConversationWindow（open-conversation-window.ts 内建）；成功=新窗口实体呈现；菜单点击即关，pending 无载体
57. conversation-context-menu.tsx:76 `conversation-menu-hide` | `hideConversation: (key, atMs: number) => void`（:36）| 无需接线（行即时消失）
58. conversation-context-menu.tsx:83 `conversation-menu-delete` | `removeConversation: (key, atMs: number) => void`（:38，仅隐藏列表项+清旗标，不删好友不退群）| 无需接线（行即时消失）

追加域结论：0 接线。六项中仅 openWindow 异步且既有失败链路完备；其余全部 fire-and-forget 同步 void（签名见上），效果即时可见，接线反致提示洪水。

## 检查点⑥ [补充] 并入（2026-09-14 23:58 -07:00）
- rebase：origin/main effd6e4f → f430e92a（越过派发时点 23c9a279，uix 波 session-sidebar 重写/acp 视觉对齐/选中态降噪已并），feature 侧零冲突，我的 2 提交重放为 4b024bc9（i18n）+ d99cfb66（接线）。
- 门禁复跑：515/515 全绿（含主干新带 15 用例）、tsc exit 0、lint exit 0。
- 追加域两文件走查 7 行（52-58），0 接线，全审计记录如上。

## 检查点⑦ 修复轮 1（2026-09-14 00:17 -07:00）
- skill 沉淀搬迁：主树未提交的 3 文件 13 行 append 撤下，改随分支提交（4c25b610 docs(skill)），主树 .agents/skills 恢复干净（SESSIONS.md 他人改动未触碰）。
- 契约修复（05adb91d）：share-admin-client.ts 扩权期内修改——adminJson 抛 AdminHttpError（status 字段，消息格式不变，兼容既有 startsWith("HTTP 422") 判断）；listWorkspaces 仅 404 容错返回空（旧 agent 无端点契约），其余上抛。调用方：local-acp-card 刷新错误 toast（context=acp-local-workspace-list）+ effect/按钮两路共通行内错误提示（acp-local-workspace-error）；share-create-dialog 弹层 workspace 拉取失败行内 role=alert（acp-share-ws-error）+ 保留回落默认工作区语义 + console.warn 留痕。
- 红绿证据：两新增失败路径用例先红（吞错契约下 toast/错误节点永不出现）后绿；404→空态存量用例保持绿（语义兼容性回归锚）。
- 门禁：聚焦矩阵 80 文件/517 测试全绿、tsc exit 0、lint exit 0。
- 教训：测试在 descriptor 异步发现完成前点击刷新，endpointUrl=null 使 load 提前 return 被 AsyncButton 记成成功——时序敏感用例必须先等端点就绪信号。

## 检查点⑧ 修复轮 2（2026-09-14 00:27 -07:00）
- 澄清：缺口②（吞错契约修复）已于修复轮 1 完成（回报 242a5779 与主控验收消息交错）；本轮按要求在新基线重做验证。
- 二次 rebase：吸收主干新增 8a0732e8（他人 docs(skill) 与本分支 skill 沉淀同文件双 append，techniques/known-issues EOF 冲突按双方保留消解；消解残留的 `<<<<<<< HEAD` 标记行由 c6d96e4d 清除，全仓 grep 零残留）。
- i18n 重复键块合并（30bbe4e8）：AF1 的 chat.feedback.session* 与本分支 chat.feedback.actions 文本级零冲突但语义重复，actions 子块并入 AF1 落在 main 的 feedback 块内，键路径全部不变，对 main 纯加法 diff。
- 新基线全门禁：聚焦矩阵 82 文件/532 测试全绿、tsc exit 0、lint exit 0、check:i18n exit 0。
- merge-base 证据：`git merge-base HEAD origin/main` = 8a0732e8750dfa8eab42c5662dc8c48ec3be2bcd = origin/main tip（8a0732e8 docs(skill)），分支 6 提交领先，ff-merge 可达。
