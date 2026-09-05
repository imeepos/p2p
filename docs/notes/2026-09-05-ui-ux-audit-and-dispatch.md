# UI/UX 审查与派单记录（2026-09-05）

角色：UI/UX 产品负责人（协调会话）。方法：三个只读审查子代理分域细读（monitor / group+acp+全局壳 / discovery+relay+settings+diagnostics+shared）+ 协调者独立机械核查（i18n 门禁、快捷键链路、ErrorBoundary、error-report 接线、空态承诺抽查）。视觉截图因 macOS 屏幕录制权限缺失不可用（环境问题，p2pctl gui screenshot 返回 CAPTURE_PERMISSION_DENIED，授权后可补像素级审查）。

基线：vitest 88 文件 582 用例全绿；check:i18n PASS（zh=en=652）；main 干净。

## 问题清单（审查定案，证据为审查时行号）

### P0
- P0-1 IME 组合态回车误发：聊天与 ACP 输入框「Enter 发送」未检查组合态，拼音选词回车把半截话发出（撤不回）。证据 components/chat/composer.tsx、acp/components/prompt-composer.tsx，全 src 无 isComposing 处理。
- P0-2 数据链路静默冻结：订阅失败骨架永挂、幂等锁失败后永不重试；轮询中断后全部数据冻结在旧值继续显示「运行中」。证据 stores/node-store.ts、components/layout/app-layout.tsx。

### P1
- ACP 权限请求无主动提醒，倒计时归零被静默拒绝（acp-view.tsx、permission-panel.tsx、store-events.ts）。
- 权限按钮全同色同级单击即生效，「始终允许」无确认（permission-panel.tsx）。
- ErrorBoundary 死路：只有重试，确定性崩溃无出路；文案硬编码双语绕过 i18n（components/feedback/error-boundary.tsx）。
- 命令面板与数字快捷键零可发现性；palette.hint 是从未渲染的死键且内容漂移（topbar.tsx、sidebar.tsx、use-hotkeys.ts、locales）。
- ACP 提示词草稿跨会话串线，可能发给错误 agent（acp-view.tsx 未按会话隔离）。
- 群聊向上翻历史视口跳位（WebKit 无 scroll anchoring，前插后未补偿 scrollTop；group-message-list.tsx）→ 归入 chat 邻接域，随 DEBT1 一并处理（PR1 合并后），本轮不动 group/message-list 以免与 IM 域并行工作冲突。
- 设置表单/资料草稿/中继地址列表脏状态切页静默丢失，全应用无路由守卫（settings-view.tsx、profile-card.tsx、relay-config-card.tsx、App.tsx）。
- 设置页保存校验失败零反馈，错误红字可能在视野外（save-bar.tsx、use-settings-save.ts）。
- mDNS 开关发现页即时落盘 vs 设置页置脏+保存，两套模型互相矛盾（mdns-card.tsx、network-card.tsx）。
- mDNS 徽章与详情做进行时表述，实际只写配置重启才生效（mdns-card.tsx、zh-CN.ts）。
- 事件页暂停计数在缓冲打满后失真（长度差不再增长，use-events-controller.ts、event-reducer.ts）。
- 节点页空态承诺不兑现：未运行时自动发现/拨号两条路都不通且无启动引导（peers-table-card.tsx、peer-dial-dialog.tsx）。

### P2
- 弹窗打开时数字热键仍切页，表单内容静默丢失（use-hotkeys.ts）。
- toast 去重命中全量 dismiss 在显提示（toast.ts）。
- ACP 关闭会话/移除端点/移除目录零确认，与群管理确认纪律不一致。
- 过滤空态无「清除筛选」恢复入口（peers-table-card.tsx、events-list-card.tsx）。
- 节点表无虚拟化且 5s 全量重渲、peers 无上限 → DEBT2。
- 拨号失败兜底英文串与事件 tooltip 原始协议串混入本地化界面（peer-row-actions.tsx、recent-event-line.tsx）。
- PeerId 截断长度 12/10/16 三处不一致（dashboard-status-cards.tsx、peer-table-row.tsx、peer-detail-sheet.tsx）。
- 仪表盘「已知节点」卡缺加载骨架（dashboard-metric-cards.tsx）。
- 趋势卡空态文案与卡片头同屏重复（dashboard-trend-card.tsx）。
- 诊断页一键清理无确认 + 三处原始错误串直出（diagnostics-view.tsx）。
- rendezvous 地址簿空态双「添加地址」按钮（rendezvous-card.tsx）。
- 逐跳比例条 fail 段被裁剪永不可见（hop-stats-card.tsx 容器非 flex）。
- 发现结果空态承诺出口无入口按钮（discovered-table-card.tsx）。
- 宣告/观测卡全行话无白话说明（advertise-card.tsx）。
- 设置页整份快照回写可回滚他处改动 + localConfig 遮蔽外部变更 → DEBT3（需设计裁决）。
- useNumberRouteHotkeys 注释「前六个」与实现不符（use-hotkeys.ts）。

## 协调裁决
1. chat 域（components/chat、views/chat、lib/chat-*）为 PR1 并行分支所有，本轮全部排除；IME 守卫在 ACP 侧先修（UI-B），chat 侧收敛共享钩子登记 UI-DEBT1。
2. 四包按文件所有权切分，互不相交：UI-A 全局壳（feat/ui-ux-shell）、UI-B ACP（feat/ui-ux-acp-console）、UI-C monitor 数据可信（feat/ui-ux-monitor-trust）、UI-D 设置域（feat/ui-ux-settings-domain）；i18n 按键块 append-only 分治；App.tsx 归 UI-D，app-layout.tsx 归 UI-C，hooks/use-hotkeys.ts 归 UI-A。
3. 群聊滚动跳位（P1）本轮不修：改动面与 IM 域并行工作相邻，登记随 DEBT1 时点处理。
4. DEBT3 需要先裁决保存模型（增量 patch vs 磁盘版本比对），未裁决不派单。
5. 验收一律协调者在主树机械复跑（区域 vitest + build + check:i18n + make check），不采信会话自报；会话只推送分支回报 tip。

## 派单记录
UI-A session-1806eb85 / UI-B session-fcf65c5a / UI-C session-04d71802 / UI-D session-2c8c8ba5（均为本轮新建专属会话，账本 status=doing）。债务：UI-DEBT1/2/3（todo）。
