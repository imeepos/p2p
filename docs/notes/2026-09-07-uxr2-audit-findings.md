# p2p GUI 体验走查审计·第二轮补盲（UX-R2，2026-09-07）

审计人：p2p GUI 体验走查审计员（AI，UX-R2 补盲轮）。仅审计、仅产出本报告，未改任何业务代码。

## 范围与基线关系

- 审计基线 main @ acca73c，工作区干净。第一轮全页审计（ec1b783 基线，28 findings）已全部整改收官（终验报告 docs/notes/2026-09-07-ux-final-walkthrough.md）。
- 本轮补盲对象为第一轮未覆盖/覆盖过浅的 6 个盲区：
  1. `/llm-share` 四面板（offer 出借 / borrow 借入 / ledger 账本 / allowlist 白名单，b37dab5 合入，晚于 R1 基线）
  2. `/network` 下的监控（monitor）页
  3. `/docs` 协议文档页
  4. 设置内 update/about 更新卡
  5. ACP 控制台深走（R1 仅 F06/F25 两条）
  6. 概览页深走（R1 仅 F19/F22 两条）
- **「监控（monitor）页」口径说明**：现行代码无独立 monitor 路由——历史上 G-C「监控视图」包产出仪表盘/节点/事件三视图（后迁入 views/network，即现 overview/peers/events 六 tab 族），`components/monitor/` 仅为 UI 原语目录。本轮按监控族处理：以 `/network/events`（事件监控）+`/network/diagnostics`（诊断监控）为主，附数据链路可信横幅（data-link-banner，monitor P0 产物）与 overview 状态监控面；概览深走另列（盲区 6）。

## 走查方式与环境备注

- mock 实例：程序化 `createServer` 复用仓库 vite.config.ts（仓库零改动），cacheDir 隔离 `/tmp/uxr2/.vite`，端口 5176，shell 显式 `VITE_MOCK_IPC=1`（规避 apps/gui/.env.development.local 的 VITE_MOCK_IPC=0 覆盖）；启动与 Chrome 均 unset HTTP_PROXY/HTTPS_PROXY/ALL_PROXY + `NO_PROXY='*'`，Chrome 加 `--no-proxy-server`。
- 驱动：scripts/gui-agent.mjs 的 /tmp 专属副本（/tmp/uxr2/agent.mjs，调试端口 9236），单 Chrome 会话内按场景顺序 nav/eval/snap，注入页侧辅助函数做表单填充（React 受控组件原生 setter + input 事件）与 DOM 断言；每场景独立 Chrome 进程，走完即杀。
- 造数：llm-share 数据以真实 UI 表单驱动（白名单加入 → 借用（二次确认）→ 账本/收据验核），全链不绕 UI；事件流数据来自 mock 节点自动启动的真实事件管线。
- 证据：DOM 断言 JSON 存 /tmp/uxr2/s1-out.json … s10-out.json（10 场景），截图 /tmp/uxr2/shots/s*.png（26 张）。所有 P0/P1 均为页面实测（DOM 断言 + 截图）；【代码推断】条目均已注明。
- 域标记沿用 R1：1 操作路径 / 2 按钮反馈链 / 3 表单 / 4 表格 / 5 选择器；本轮按任务书另加两条通用线：6 文案（用户语言，不得泄漏内部术语）与 7 空态（必须给出路）。优先级口径沿用 R1：P0=反馈链路断裂、删除无确认、选择器不可用/不一致、高频操作路径过长、表格显示内部编号；P1=明显摩擦或一致性缺陷；P2=打磨项。

## Findings（26 条）

| # | 页面 | 域 | 实际操作路径与现状 | 证据 | 建议改法 | 优先级 | 方式 |
|---|---|---|---|---|---|---|---|
| R2-01 | /llm-share·账本 | 2 | 借入方在 borrow 面板完成借用（确认弹窗→「借用完成/已入账 true」报告卡出现）后，同页下方「双边账本」：净差卡仍显示「本机未参与任何流水」、流水表「暂无流水记录」（实测 rows=0）。点流水区「查询」后流水出现，但净差卡**依旧**停留旧空态——净差卡仅挂载时拉取一次，无任何刷新入口，「查询」也不带动它 | /tmp/uxr2/s2-out.json（ledger-after-borrow-no-refresh、ledger-apply-filter）、s10-out.json（有流水+已查询，balanceRows 仍空）、截图 s2-borrow-report.png、s2-ledger-after-query.png | 借用成功后联动刷新三卡（或收敛到共享 store 订阅）；净差卡补显式刷新按钮；「查询」同时刷新净差 | **P0** | 页面实测 |
| R2-02 | /llm-share·白名单 | 2/4 | 白名单行「移出」点击即生效：实测 200ms 内行消失、全程无任何确认弹窗（before=2 → rowsRightAfter=1，dlgAppearedDuring=false）。移出即撤销该借方借用授权，属破坏性操作；对照全站删除/停用均有 alertdialog 确认（同页 borrow 二次确认、Agent 端点移除确认先例俱在） | /tmp/uxr2/s2-out.json（deny-click-check）、截图 s2-after-deny.png | 走 ConfirmProvider 二次确认，文案说明后果（该借方立即不可借用；如需恢复可再次加入） | **P0** | 页面实测 |
| R2-03 | /llm-share·offer 空态 | 6/7 | 新用户首屏：offer 面板空态标题「尚未发布能力声明」下，描述直显内部英文报错原文 "never published: run offer publish first"。代码里已写的中文引导「签名发布后写盘 offer.json 并对借方可见」被 description={loadError 作为兜底} 永远覆盖（未发布必然抛错→loadError 恒非空），空态即报错、无路可循；控制台同步留 warn | /tmp/uxr2/s1-out.json（offer-panel-text）、s1/s2/s3 consoleLogs、offer-panel.tsx EmptyState、mock-backend.ts offerShow | 「未发布」是常态非故障：识别该语义走中文空态文案（给出「在上方表单签名发布」的路）；仅真实错误才显示错误原文 | **P1** | 页面实测 |
| R2-04 | /llm-share·白名单 | 4 | 表格三连：表头硬编码英文 "PeerId"（未经 i18n）；主列显示原始 44 位 PeerId（好友昵称/备注不回填，对照 R1 F02 网络族已修标准）；「授权时间」显示原始 ISO UTC "2026-09-07T01:42:40.000Z"（非本地时区、非人话格式） | /tmp/uxr2/s2-out.json（allow-real-peer：headers/rows 原文）、截图 s2-allow-real-peer.png | 表头走 i18n「借方」；已知好友行显示昵称+PeerId 缩略（复用 peer-name 组件）；时间走 formatDateTime 本地化 | **P1** | 页面实测 |
| R2-05 | /llm-share·白名单/借用 | 3/5 | PeerId 关联输入全为自由文本、无选择器：「借方 PeerId」（allowlist）与「出借方 PeerId」（borrow）均无「从发现清单/节点表选择」选择器（entity-combobox/dial-peer-picker 组件现成）；且均无格式校验——实测 "alice-fake-peer" 直接进入白名单成行、"zzzNotAllowlisted99" 可一路走到借用确认弹窗。R1 F03 家族在 llm-share 新页面回潮 | /tmp/uxr2/s2-out.json（allow-short-invalid-peer）、s1 form-inventory | 复用全局节点选择器；自由文本保留兜底但加 base58/32 字节即时校验（同添加好友表单） | **P1** | 页面实测 |
| R2-06 | /llm-share·借用 | 3 | borrow 表单 model/maxTokens/messages 三个必填字段均无占位符、无辅助说明（实测 placeholder 全 null）；model 实为可枚举输入——本机白名单已放行的模型集就是现成选项源——却用自由文本让用户手打 | /tmp/uxr2/s1-out.json（borrowFields）、s1-llmshare-init.png | model 改选择器（数据源=allowlist 模型集，留自由输入兜底）；maxTokens/messages 补占位与辅助说明 | **P1** | 页面实测 |
| R2-07 | /llm-share·全局 | 6 | 内部术语/内部格式泄漏清单（均为页面实测 DOM 原文）：① 按钮「重试（复用 reqId）」；② 报告卡字段名 "disputeWindowSecs" 配裸秒数 86400；③「已入账 false/true」裸布尔；④「正文预览（截断）」标签下成功态展示上游 SSE 原始 JSON 片段、拒绝态展示英文内部句 "upstream untouched: borrow rejected structurally…"；⑤ 拒绝码 not_allowlisted 裸直出无一行人话解释；⑥ 白名单空态标题 "allowlist 无条目即不可用"；⑦「净差视图（lender + 账期）」「lender 过滤/borrower 过滤」与流水表头 "Lender/Borrower" 英文角色术语；⑧ offer 标签「闲量（model=N）」「单请求上限（可选，model=N）」把语法片段放标签；⑨ 报告卡「SSE 帧数」 | /tmp/uxr2/s1-out.json（panels text）、s2-out.json（report/rows）、s3-out.json（rejected report）、截图 s3-borrow-rejected.png | 逐条改用户语言：请求编号、72 小时争议窗、入账/未入账、出借方/借入方、SSE 事件数；拒绝码保留但补一行人话原因，码入可复制详情（F06 先例） | **P1** | 页面实测 |
| R2-08 | /llm-share·offer | 2 | 「签名发布」成功后无显式确认（无 toast），仅下方状态卡静默出现；对照全站保存类操作均有 toast（中继/资料保存） | /tmp/uxr2/s3-out.json（offer-publish-success：toast 区仅有无关的更新提醒）、截图 s3-offer-published.png | 成功补 toast「能力声明已发布，借方可见」或状态卡出现时轻微强调 | P2 | 页面实测 |
| R2-09 | /llm-share·offer | 6 | 状态卡「剩余秒数 1」「剩余秒数 0」裸秒倒计时；过期后仍显示「剩余秒数 0」而非「已到期」语义 | /tmp/uxr2/s3-out.json（offer-refresh-expired）、截图 s3-offer-expired.png | 人性化时长（如「59 分钟后到期」/过期显示「已于 HH:mm 过期」） | P2 | 页面实测 |
| R2-10 | /llm-share·账本 | 4 | 流水表无「发生时间」列（i18n 已有 columnTs「时间」键未渲染，实测表头 7 列无时间）；亦无行数/合计 tokens 等统计行，检索结果规模不可见 | /tmp/uxr2/s2-out.json（ledger-apply-filter：headers 7 列）、ledger-entries.tsx | 补时间列与「共 N 条 · 合计 X tokens」统计行 | P2 | 页面实测+代码推断 |
| R2-11 | /llm-share·账本 | 4 | 净差行组合值 "（1801/-0, 2）"（lentOut/-borrowed, entries）无字段说明；lender 显示裸 PeerId 无昵称映射 | ledger-balance.tsx BalanceRow；净差行因 R2-01 之故 mock 实测不可达 | 组合值拆列或加标签（出借 X / 借入 Y · N 笔）；lender 走昵称映射 | P2 | 【代码推断】 |
| R2-12 | /llm-share·表单 | 3 | 三面板校验错误均只有 role=alert 文本，字段无 aria-invalid/aria-describedby 关联（offer 空提交后 7 个字段 aria-invalid 仍为 null，实测；allowlist/borrow 同型）。R1 F24 同族 | /tmp/uxr2/s1-out.json（offer-empty-submit aria 数组）、截图 s1-offer-empty-errors.png | 补 aria 关联与聚焦播报（复用 focus-first-error 既有设施） | P2 | 页面实测 |
| R2-13 | /llm-share·全局 | 1 | 入口发现性：rail 6 入口无 llm-share，仅设置入口卡+命令面板「LLM 共享」可达；对照 /docs 曾以 F15 同样理由升 rail | /tmp/uxr2/s1-out.json（railHrefs 6 条、palette items 含「LLM 共享」）、截图 s1-llmshare-init.png | 评估升 rail（或至少进顶栏直达），注意 menu.def append-only 约束压独立小提交 | P2 | 页面实测 |
| R2-14 | /llm-share·借用 | 2 | 借用提交处理中仅按钮 disabled（无 loading 文案/进度语义）；LLM 调用为长耗时动作，等待期用户无「处理中」感知（确认弹窗关闭后到报告卡出现之间） | borrow-panel.tsx（plain Button disabled={busy}，未用 AsyncButton loadingLabel） | 换 AsyncButton 或加行内「正在调用…」状态行 | P2 | 【代码推断】 |
| R2-15 | /llm-share·offer | 3 | 表单「留存自述」（retention）字段未渲染：locale 有 formRetention「留存自述」键、EMPTY_OFFER_FORM.retention 默认 "none" 直接随请求提交；出借方无法在 GUI 声明留存策略（隐私相关自述） | offer-panel.tsx（无该字段）、offer-form.ts、zh-CN.ts formRetention | 补渲染该可选字段（文本输入+用途说明） | P2 | 【代码推断】 |
| R2-16 | /network/events | 4 | 事件行「详情」展开为原始 JSON 直排（"type": "peer_discovered" / 完整 44 位 PeerId 原文），开发者向格式无折叠 | /tmp/uxr2/s9-out.json（events-detail-expand2 sample） | 详情 JSON 默认折叠，提供「复制详情」（对照 R1 F27 口径） | P2 | 页面实测 |
| R2-17 | /network/overview·最近事件 vs /network/events | 4 | 同一 PeerId 两处截断样式不一：概览「节点已连接 vKLTAv6c」（前 8 位）vs 事件页「发现节点 vKLTAv…XPUV」（前 6+后 4） | /tmp/uxr2/s5-out.json（overview/events bodyText）、s6 | 统一缩略组件（建议前 6…后 4+悬停完整+复制） | P2 | 页面实测 |
| R2-18 | /network/overview | 2 | 趋势区 4 个 sparkline svg 仅 2 个带 aria-label/role，半数读屏不可达 | /tmp/uxr2/s6-out.json（trend-cards-a11y） | 补 aria-label（指标名+当前值） | P2 | 页面实测 |
| R2-19 | /network monitor·数据链路横幅 | 2 | 数据链路可信横幅三态（引导失败/数据过期+重试按钮）在 mock 态无法触发（引导恒成功），live 页面未实测到；组件与 store 派生逻辑在位，恢复后自动消失的设计合理 | data-link-banner.tsx、node-store | 无需改动；建议补一条 e2e：注入 bootstrap 失败断言横幅出现与重试 | P2 | 【代码推断】 |
| R2-20 | /docs | 2 | 文档内链接点击零反馈：实测点击 "../design/wire-protocol.md" 链接——不跳转、无 toast、无任何视觉变化（拦截仅落 console.warn "[docs] 文档链接暂不支持应用内跳转"）；且链接渲染文本就是内部仓库相对路径原文 | /tmp/uxr2/s4-out.json（docs-link-click-noop：navigated=false + console warn 原文）、截图 s4-docs-overview.png | 拦截时给出用户可见反馈（toast+复制路径）；站内可达的跨文链接映射到五篇目录；外链在桌面端走系统浏览器 | **P1** | 页面实测 |
| R2-21 | /docs | 1 | 长文无页内导航：quickstart 正文高约 3446px（约 5 屏）仅有五篇级左目录，无小节锚点/返回顶部 | /tmp/uxr2/s4-out.json（docs-switch-article scrollH=3446） | 渲染 H2/H3 锚点目录或加返回顶部 | P2 | 页面实测 |
| R2-22 | 设置·更新卡 | 2 | 「跳过此版本」无确认直接生效（实测点击后立即出现「已跳过版本 0.2.0，将不再提醒」）；且界面无撤销入口（仅一行小字告知，无「取消跳过」按钮） | /tmp/uxr2/s4-out.json（update-skip-version）、about-update-card.tsx | 已跳过行加「取消跳过」入口；或首次跳过轻确认 | P2 | 页面实测+代码推断 |
| R2-23 | ACP 控制台（chat agent 形态） | 2 | 连接失败后的恢复指引指向不存在的控件：实测点「连接」失败后文案两处指路——「可在『高级设置』手动补 Peer ID」「请在连接卡里补全（Token 在高级设置）」，但当前 chat agent 面板与全站均无「高级设置」/「连接卡」控件（实测 advancedControls 为空；真实编辑路径在「通讯录→Agent→本机 agent→详情→编辑」，该弹窗中 Token/PeerId 字段平铺直出、并无「高级设置」分组）。旧 AcpView 时代文案残留，手动恢复链最后一步落空 | /tmp/uxr2/s7b-out.json（agent-connect-click、agent-error-copy-detail、agent-advanced-settings-search）、s8b-out.json（编辑弹窗全文）、截图 s7b-agent-initial.png | 文案指向真实路径（「通讯录 → Agent → 本机 agent → 编辑」）并在失败卡加直达链接；同步 error-help 文案源 | **P1** | 页面实测 |
| R2-24 | ACP 控制台 | 2 | 旧版 AcpView 整树（连接卡/会话侧栏/ConfigPanel/UsageBar/PermissionPanel/分享管理/加入卡/permission-notice-bridge）未挂接任何路由：App.tsx 仅 /acp→/chat?kind=agent 重定向，AcpView 引用仅存测试与 routes/acp-page.tsx 再导出（该文件仅被测试 import）。live 界面的 agent 在线形态只挂 Transcript+PromptComposer——权限请求应答、模型/思考档配置、用量条在 chat 形态的落点需产品确认；若确缺，属能力回退而不仅是 UX 瑕疵 | grep 证据：AcpView 引用清单、App.tsx 路由表、agent-conversation.tsx 在线分支仅 Transcript+PromptComposer | 产品拍板：要么把 AcpView 恢复挂载（如 /chat?kind=agent 的管理态），要么把权限应答/配置/用量显式补进 chat agent 形态并删除死代码 | **P1** | 【代码推断】（grep 结构证据；mock 态无法触达权限请求验证 live 表现） |
| R2-25 | 命令面板（全局） | 6 | 面板底部快捷键提示过期：「Cmd/Ctrl+1..4 切换一级入口」，实际 rail 已是 6 项（F15 升级后未同步）；旧键 palette.hint「Cmd/Ctrl+1..8」为无引用死键残留 | /tmp/uxr2/s9-out.json（palette-footer 原文）、command-palette.tsx、zh-CN.ts | 提示与 rail 数量同源（1..6）；清理死键 | P2 | 页面实测+代码推断 |
| R2-26 | 全局（浏览器 mock 态） | 2 | console 噪音：每个会话一条 "[acp] console status 不可达 http://127.0.0.1:8788/discovery TypeError: Failed to fetch"（console-watch 发现面轮询，全站一次性非逐页）；llm-share 首载另有一条 "[llm-share] offer show 失败" warn（与 R2-03 同根） | /tmp/uxr2/s1–s10 各 out.json consoleLogs 汇总 | 无 Tauri/mock 态降级为静默或单次提示（F26 同口径） | P2 | 页面实测 |


## 正向确认清单（无需整改）

- **llm-share 借用确认弹窗**：二次确认明示真实成本（「这是真实成本动作…模型 gpt-4o，maxTokens 上限 100，出借方 …」），取消/确认齐备（实测）。
- **llm-share reqId 幂等语义**：重试复用同 reqId、重放返回 appended=false 不双记账；「新请求」完整重置表单与报告（实测）。
- **llm-share 表单校验**：offer 必填集/闲量覆盖/引用未声明模型/KV 格式逐字段 role=alert 即时提示（除 R2-07⑧ 外文案为用户语言）（实测）。
- **收据验核卡**：高级字段（lenderPubkey）默认折叠+aria-expanded；验签 PASS/FAIL 徽章明确（实测 PASS 全链）。
- **设置入口卡**：设置页 LLM 共享卡一句话说明+「打开」按钮可达（实测）。
- **事件监控页**：筛选按域分组带命中计数、「仅错误/清除筛选/暂停滚动/导出 JSON/清空」齐备；事件消息人话化（「发现节点 vKLTAv…XPUV（192.168.81.6/37854）」）；详情按钮逐行显式（R1 F16/F20 修复保持）（实测）。
- **诊断页**：浏览器 mock 态「日志文件为桌面端能力」说明性空态诚实，不裸报错（实测）。
- **数据链路横幅**：引导失败/数据过期两态均带重试入口且恢复自动消失（代码在位，mock 无法触发相位——见 R2-19）。
- **/docs**：五篇目录 aria-current 正确、切换即时、表格/代码块样式正常、正文单源 docs/protocol 无漂移（实测）。
- **设置更新卡**：发现新版 toast（8s 自动消失）→ 卡内详情（发布名/Markdown 说明/发布时间本地化「2026年9月1日 03:00」）→「下载并安装」进度→「已下载并安装，重启后生效」+「立即重启」全链实测贯通；手动检查三态+失败 toast+重试；超长说明截断提示在位（实测）。
- **ACP（chat agent 形态）**：连接失败原因为人话+「复制详情」（title 含 error=endpointIncomplete 与 ws 地址，F06 修复保持）；会话列表单一状态「本机 agent 未连接」（F07 保持）；Agent 编辑弹窗字段齐整、权限策略分档清楚、危险区明示「删除将同时移除本地会话记录索引」（实测）。
- **概览**：状态卡与顶栏停止节点确认文案一致（F04 保持）；停止后「已停止，身份保留」（F19 保持）；启动无确认（合理，非破坏性）；趋势/拨号链空态「暂无拨号记录」有因；排障入口三卡与「查看全部」直达（实测）。
- **控制台纪律**：全站无逐页 console 噪音（F26 保持），仅余 R2-26 两条一次性 warn。

## 汇总计数

| 优先级 | 数量 | 条目 |
|---|---|---|
| P0 | 2 | R2-01（借入→账本闭环断链）、R2-02（白名单移出无确认） |
| P1 | 8 | R2-03、R2-04、R2-05、R2-06、R2-07、R2-20、R2-23、R2-24 |
| P2 | 16 | R2-08、R2-09、R2-10、R2-11、R2-12、R2-13、R2-14、R2-15、R2-16、R2-17、R2-18、R2-19、R2-21、R2-22、R2-25、R2-26 |
| 合计 | 26 | P0 2 / P1 8 / P2 16 |

分布：llm-share 15 条（R2-01…R2-15）、监控族与全局 6 条（R2-16…R2-19、R2-25、R2-26）、docs 2 条、更新卡 1 条、ACP 2 条（R2-23、R2-24 含概览之外的深走结论）。实测 21 条、【代码推断】5 条（R2-11、R2-14、R2-15、R2-19、R2-24）。

## Top 先行项

1. **R2-01**：借用成功后账本三卡联动刷新——打通「出借→借入→账本出记录」的业务闭环。
2. **R2-02**：白名单「移出」补二次确认（对齐全站危险操作纪律）。
3. **R2-23/R2-24**：ACP 失败指引指向真实编辑路径，并拍板 AcpView 死挂载的取舍。
4. **R2-03**：offer 空态去英文内部报错、给中文出路。
5. **R2-05**：llm-share 两个 PeerId 输入接入全局选择器与格式校验。

## 复核产物索引

- DOM 断言：/tmp/uxr2/s1-out.json … /tmp/uxr2/s10-out.json（10 场景，含逐步 label/value）
- 截图：/tmp/uxr2/shots/ 下 s1-llmshare-init、s1-offer-empty-errors、s2-allow-short-peer、s2-allow-real-peer、s2-borrow-confirm、s2-borrow-report、s2-ledger-after-query、s2-borrow-rejected、s2-after-deny、s3-borrow-rejected、s3-offer-published、s3-offer-expired、s4-docs-overview、s4-docs-second、s4-update-available、s5-download-installed、s5-events、s5-diagnostics、s5-overview、s6-stop-confirm、s6-stopped、s7-acp-initial、s7b-agent-initial、s7b-agent-after-connect、s8-agent-edit、s8b-agent-advanced（共 26 张 png）
- 走查驱动（仓库零改动）：/tmp/uxr2/agent.mjs、/tmp/uxr2/dev-server.mjs、/tmp/uxr2/s*.mjs
- 走查实例：vite dev @ http://localhost:5176（cacheDir /tmp/uxr2/.vite，VITE_MOCK_IPC=1）；收尾已停 dev server 与全部调试 Chrome，进程零残留。
