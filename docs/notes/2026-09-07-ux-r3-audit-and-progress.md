# UX-R3 第三轮体验打磨：审计汇总与进度（2026-09-07）

审计方式：四个并行只读审计（表格族/表单族/设置网络全局/WX1 聊天族），静态 file:line 证据，已对照 R1（F01–F28）与 R2（R2-01…R2-26）去重。合并后去重共约 63 条新发现：P0×1 / P1×8 / P2×54。

## 本轮已修复（feat/ux3-polish 分支，合入 main 前经全量门禁）

| 提交 | 内容 | 对应 findings |
|---|---|---|
| 62f67e9 | 全局选择器：触发器 id 绑定（Label htmlFor 全站生效）、选中/Esc 焦点归还、Esc 不穿透外层 Dialog、MultiSelect 三态共享 PickerStatusRow | A1、A6、A4 |
| c783e60 | group-store 好友簿加载失败可见化（friendsLoading/friendsError），建群/群邀请选择器三态接线 | 红线（失败可观测）、A4 |
| afff50b+697fda0 | 选择器清空语义落地（5 调用点 null 不再丢弃；拨号带入 clearable=false）、模型选择器语境文案、MultiSelect 搜索框 id、缩略统一 6+4（删 picker 第二套实现） | A2、A5、A3、N2② |
| 38d36eb | 群发送反馈链：pending 显发送中、failed 走失败+重发（onRetry 透传+群重发 handler）、报告级失败 toast 群 1:1 统一、分享 ACP 假成功修复、pending 文案「等待对方上线」→「发送中…」 | W1-01（P0）、W1-02、W1-07 |
| b23c40a | 事件页搜索同源化（人话摘要+完整 PeerId+原始兜底）、节点表搜索纳入昵称、聊天时间跨天回补（昨天/日期）、入群弹窗群主人话 | N1、N-01、W1-03、W1-04 |
| fb1b1e5+a048340+767ad7f | CopyButton 移入 feedback；共享 CommandErrorText；10 处对话框/表单内联失败原因接入复制链 | C1、N6（部分） |

新增测试：picker 组件层（关联/焦点/Esc 穿透/三态/清空）、group-store ensureFriends 失败重试、群发送状态三态、formatConversationTime 跨天、CommandErrorText、peer-id-field 清空联动。

## 剩余 backlog（后续轮次按序消化）

### S5 等待态与行内反馈
- N3/N-09/C2/N4：AsyncButton loadingLabel 清单（peer-row-actions 挂断/拨号/Ping、relay-config-card、add-address-dialog、group-invite-section 同意、messages 两类邀请行同意/拒绝/撤回、load-state）；好友邀请 accept 已是 AsyncButton 但 action 吞错失败亮成功勾（act 抛错走 onError）。
- N-04/C4：contacts 好友区撤回邀请 void cancelInvite 失败静默（红线），补行内错误+AsyncButton。
- N5：chat-store.loadInvites 失败落 invitesError，messages 好友邀请区假空态改错误行+重试。
- W1-06：1:1 重发按钮重试期间禁用，防连点重复发送。
- W1-13：composer 发送钮 loadingLabel；表情按钮 aria-expanded+面板 Esc/外点关闭。

### S6 表格人话
- N-02 收件箱裸 PeerId、N-03 入群邀请行补邀请人、N-06 流水表 lender/borrower 昵称+reqId 复制、N-07 借用报告 reqId title+复制与 SSE 预览展开、N-08 messages 时间跨天、N-12 白名单行复制+列 truncate、N-15 诊断时间本地化+kind 中文、N8 topbar PeerId 缩略+title、N-05 删死组件 PeerIdCell。
- N-13 白名单检索/计数/成功 toast/deny 行 pending；N-17 节点表与事件表统计行。

### S7 表单 aria/校验
- B1/B2/N9：endpoint 表单族与 AddressListEditor 校验错误 id+role=alert+aria 关联+失焦校验（对齐 PortField/friend-add 范式）；B3 建群名 aria+失焦时机；B4/N13 重置身份确认输入 Label/aria-label+maxLength=4；B5 policy-editor 下拉 aria-labelledby（config-panel 范式）；B7 offer rpm/concurrency/ttl inputMode/占位/单位（maxTokens 范本）；B8 retention 占位 none 人话化；C3 agent 抽屉编辑取消/校验/成功反馈；N10 中继卡保存失败定位+计数；N-19 Agent「发消息」disabled Link 不阻断导航。

### S8 空态/一致性/产品拍板项
- N-11 messages/净差/事件空态出路；W1-09/10 消息流与群兜底空态出路；W1-11 邀请选择器候选空态分态；W1-12 左栏 section aria-label；W1-14 会话行状态图标 aria 与 time dateTime；W1-15 WX1 一致性残留；N11 横幅错误 title+复制；N12 profile-card AsyncButton；N14 净差行点击联动流水过滤；N16 诊断复制日志+暂停；N7 端口解析收敛一处；N-20 out 向邀请行跳转兜底确认；N-21 白名单移出确认拼关联流水数。
- W1-05（产品拍板）：会话行右键菜单/置顶/免打扰；气泡复制；好友头部查看资料直达。W1-16/D1：GroupView/GroupCreateDialog 死挂载拍板（删除或恢复）。
- W1-17：WX1 新增面（会话行视觉/重发流/mark_failed toast）回归测试防线补齐。
- 注释清理：palette-nav.ts / use-hotkeys.ts 过期计数注释。

## 验证口径

- 分支门禁：eslint / tsc -b / vitest 全量（当前基线 1150+ 用例）/ pnpm build / bash scripts/check/i18n-diff.sh。
- 页面实测：VITE_MOCK_IPC=1 + scripts/gui-agent.mjs 逐页走查（本轮走查记录为后续轮次待办，先以全量用例+构建门禁托底）。
