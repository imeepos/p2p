# UX-K 操作路径一致性 交付报告（F08/F09/F10/F15/F22/F26/F27）

日期：2026-09-07　分支：feat/uxk-flow-consistency　worktree：.worktrees/feat-uxk-flow-consistency
基线：main @ 706fafc（任务书基线 4690a36 之后含波1 收官提交），合并 origin/main @ 986cf1c（UX-J）后门禁复跑全绿。

## 分支 tip

合并后 tip 见 `git log --oneline -1`（本报告提交为准）。提交序列（每条可独立 revert）：

1. feat(gui-rail): 消息中心/协议文档升 rail 常驻入口(F15)
2. i18n(uxk): 新增 UX-K 命名空间并统一建群/概览用户文案
3. fix(gui-bridge): 无 Tauri 环境路由上报降级单次提示(F26)
4. feat(gui-diagnostics): 无桥说明性空态与堆栈折叠复制(F27)
5. feat(gui-messages): 发出的邀请卡补撤回入口(F08)
6. feat(gui-contacts): 建群一跳直达与空好友引导(F09/F10)
7. refactor(gui-palette): 网络总览三名统一为概览(F22)
8. test(gui-hotkeys): 数字热键上界断言随注册表扩展修正(F15)

## 变更文件清单

- F15：`apps/gui/src/config/menu.def.ts`（append-only 追加 /messages /docs）、`components/layout/icon-rail.tsx`（消息中心角标，与铃铛同源 selector）、`icon-rail.test.tsx`、`hooks/use-hotkeys.test.tsx`、`views/messages/messages-page.tsx`（注释）
- F26：`lib/tauri-env.ts`（新增）、`lib/control-bridge.ts`、`lib/control-bridge.test.ts`
- F27：`views/network/diagnostics/diagnostics-view.tsx`、`diagnostics-cards.tsx`（新增）、`diagnostics-view.test.tsx`
- F08：`views/messages/friend-invite-section.tsx`、`messages-page.test.tsx`
- F09/F10：`views/group/group-create-form.tsx`（新增）、`group-create-dialog.tsx`、`views/contacts/group-add-dialog.tsx`、`group-section.tsx`、`contacts-group-flow.test.tsx`
- F22：`config/palette-nav.ts`、`test/network-tabs.test.tsx`、`test/app-boot.test.tsx`
- i18n：`i18n/locales/zh-CN.ts` / `en-US.ts`（uxk 命名空间 +9 键；group.create.namePlaceholder、network.overview.title 值更新；zh=en 键集恒等）

## 四项门禁摘要（合并 origin/main 后复跑）

| 门禁 | 结果 |
|---|---|
| vitest 全量 | 170 文件 1026 用例全绿（基线 1008 + 本卡新增 18 + 并行波增量） |
| lint | eslint 0 告警 0 错误 |
| tsc | tsconfig.app --noEmit 零错 |
| check:i18n | PASS（zh=1052 en=1052，键集恒等） |

## 逐 finding 实测证据（mock dev 实测：VITE_MOCK_IPC=1、vite dev :5187、CDP 专属端口 9245、每场景单 target 闭环）

### F08 消息中心发出的邀请卡补撤回
操作路径：#/contacts?add=<peerId> 发起好友邀请（out pending）→ #/messages 好友邀请组 → out 向卡片行内出现「撤回」按钮（标签与通讯录同源 i18n 键 contacts.friends.cancelInvite）→ 点击 → 卡片消失回到「暂无好友邀请」空态。
结论：修复。两处同卡同源（同 store action chatInviteCancel + 同 i18n 标签），撤回失败原文行内上浮（单测覆盖）。
截图：/tmp/uxk-f08-withdraw-card.png（卡片带撤回按钮）、/tmp/uxk-f08-messages.png（撤回后空态）。
说明：入群邀请 out 向无撤回——IPC 契约层无 chat_group_invite_cancel 命令（数据层不支持），审计所指「通讯录同卡有撤回」即好友邀请卡，本卡不越界造数据层命令。

### F09 建群一跳直达 + 收到的邀请降页签 + 无好友引导
操作路径：#/contacts 群区「添加群聊」→ 弹框默认页签直接呈现建群表单（无二选一中转，少一跳）；「收到的入群邀请」为次级页签，mousedown 激活后如实呈现已入群清单；mock 无好友时表单位置呈现「先去添加好友」CTA（走 /contacts?add= 直开加好友弹窗）。
实测：dialogOpen=true、emptyFriendCta="先去添加好友"、invitesTab 切换后 aria-selected=true 且清单可见。
结论：修复。group-view 原建群入口签名不变（GroupCreateDialog 复用 GroupCreateForm），两处一跳直达。
截图：/tmp/uxk-f09-group-cta.png。

### F10 建群文案去内部术语
实测：全站 11 路由扫描「trim 后 1-64」「仪表盘」「网络概览」零命中；占位符精确值由组件单测断言（“输入群名（1-64 个字）”）；超长群名由前端前置校验就地提示（MAX_GROUP_NAME_CHARS，不再依赖文案承载规则）。
结论：修复。说明：mock dev 好友簿恒空（邀请不可自闭合），建群表单在纯浏览器 mock 下呈现为 CTA 态，占位符 DOM 断言在组件测试层完成。
截图：/tmp/uxk-f10-words.png（全站扫描伴随截图）。

### F15 消息中心/协议文档 rail 常驻
操作路径：任意页 → rail 自上而下：聊天/通讯录/网络/消息中心/协议文档/设置（设置仍沉底）；消息中心入口与顶栏铃铛同源角标（两类 in 向 pending 之和）；Cmd+1..6 随注册表扩展（5/6 为既有保留段的合法落点）。
实测：nav 六链接 href/aria-label 逐一断言通过，角标由单测覆盖。
结论：修复。⌘K 与顶栏铃铛保留为快捷方式。
截图：/tmp/uxk-f15-rail.png。

### F22 网络总览一名
操作路径：#/network/overview → 页头 h1「概览」、tab 首项「概览」；⌘K 打开命令面板 → 面板项「概览」（与页头同源 i18n 键 network.overview.title）；「仪表盘」全站零命中。
结论：修复。
截图：/tmp/uxk-f22-overview.png。

### F26 control-bridge 无 Tauri 降级
操作路径：浏览器 mock dev 下加载应用并连续切换 8 条路由 → console 无任何 control-bridge 路由上报失败（修复前逐路由 warn 刷屏）；非 Tauri 环境下安装时单次 console.info 说明后不再注册监听（能力缺失留单点信号，不静默）。
实测：runtime.console 零 control-bridge 命中、零异常（剩余 warning 为 F05 范畴的 react-router blocker 提示，基线已知，非本卡）。
结论：修复。
截图：/tmp/uxk-f26-console.png。

### F27 诊断页桌面端空态与堆栈折叠
操作路径：#/network/diagnostics（浏览器 mock dev）→ 环境卡/日志尾卡显示「日志文件为桌面端能力」说明性空态（无禁用复制按钮无说明的旧态、无原始堆栈直出）；5s 轮询停用（不再逐轮 toast 报错）；前端错误缓冲照常展示，堆栈折叠为「复制详情」（单测断言剪贴板内容含消息+堆栈）。
结论：修复。
截图：/tmp/uxk-f27-diagnostics.png。

## 边界与移交

- 群邀请 out 向撤回需数据层新增 chat_group_invite_cancel 命令，超出本卡文件所有权，未实施；如需补齐建议单开卡（IPC + Rust 侧 + 两处卡片复用本卡撤回交互）。
- App.tsx 头注「MENU_ENTRIES 四入口」字样已过时（现为 6），该文件为他人注册面，仅在此登记未动。
- .env.development.local 的 VITE_MOCK_IPC=0 本地覆盖仍存在（审计环境备注 1），走查以 shell 显式 VITE_MOCK_IPC=1 独立实例完成。
