# p2p GUI UX 终验走查报告（2026-09-07）

终验走查人：UX 终验走查员（AI，独立验收会话）。仅走查出报告，未改任何代码/配置。

## 验收对象与环境

- 对象：main @ 7ea7834（apps/gui，已合入 UX-E/F/G/H/I/J/K）。实测分支 HEAD 为 0d94c9e，`git diff 7ea7834..0d94c9e -- apps/gui` 为空，二者 apps/gui 代码等价，验收结论适用于 7ea7834。
- 实例：独立 vite dev 实例（`VITE_MOCK_IPC=1`，端口 5188，cacheDir 隔离至 /tmp/final-walk/.vite，仓库零改动；程序化 createServer 复用仓库 vite.config.ts）。启动命令显式 `unset HTTP_PROXY …; export NO_PROXY='*'`。
- 驱动：scripts/gui-agent.mjs 的 /tmp 专属副本（调试端口 9223→9235），新增 flow 命令：单 Chrome 会话内逐场景「新 target → 导航 → 单次 eval 闭环（注入+导航+操作+断言）→ 截图 → 关 target」，场景间 mock 态互不污染。
- 造数：好友/群数据经页面内 `import('/src/lib/mock-ipc.ts')` 拿 mockBackend 直建（chatFriendAdd 测试引导入口 / UI 邀请制流程 / `__MOCK_GROUP__` 系列）；PeerId 用页内 base58(32 字节) 编码以满足表单「解码 32 字节」校验。全部 28 条均为页面实测（DOM 断言 + 截图），无【代码推断】项。
- 截图：/tmp/final-walk/shots/f01.png … f28.png（含 f08f21.png、f09b.png、debug2.png 辅助证据）。

## 结论速览

| 结论 | 数量 | 条目 |
|---|---|---|
| 已修复 | 25 | F01 F02 F03 F04 F05 F06 F07 F08 F09 F10 F11 F12 F13 F15 F16 F17 F18 F19 F20 F22 F24 F25 F26 F27 F28 |
| 部分修复 | 2 | F21 F23 |
| 未修复 | 1 | F14 |

## 逐条验收

| # | 结论 | 实测操作路径与证据（截图 /tmp/final-walk/shots/） | 未修复/缺口现象 |
|---|---|---|---|
| F01 | 已修复 | 新会话落 #/chat：空态文案「会话列表目前只有本机 Agent；添加好友，对方同意后即可开始私聊」，双 CTA「添加好友」「去通讯录」均可见；点「添加好友」拉起同款添加好友弹窗（f01.png） | — |
| F02 | 已修复 | 注入好友并连接后 #/network/peers：首列「走查好友甲」，PeerId 缩略副行「xy5gYv…ESu5」，行内保留 挂断/Ping/详情 与复制完整 ID 入口（f02.png） | — |
| F03 | 已修复 | 添加好友弹窗含「从发现清单/节点表选择」选择器，PeerId 自由文本兜底仍在（f03.png）；手动拨号弹窗选择器见 F11（dial-peer-picker），ACP 弹窗辅助选择器见 F25 | — |
| F04 | 已修复 | 顶栏「停止节点」点击弹「停止节点？…确定继续吗？ 取消/确认」（与状态卡 StopNodeDialog 同款），双入口确认一致（f04.png） | — |
| F05 | 已修复 | 中继「地址 1」脏改后直改 hash：拦截弹窗「放弃未保存的修改？…留在本页/放弃修改」，hash 保持 relay；history.back() 同样被拦（f05.png） | — |
| F06 | 已修复 | 造缺 Token 端点后点「连接」：行内文案「未能从 console 发现面定位本机 agent 节点：可在「高级设置」手动补 Peer ID」，无内部错误码直显（f06.png） | — |
| F07 | 已修复 | 会话列表 agent 条目仅显示「本机 agent 未连接」单一状态，无「未连接 未连接」重复拼接（f07.png） | — |
| F08 | 已修复 | UI 发邀请后消息中心「发出的」卡含「撤回」按钮（邀请对象乙｜撤回｜待处理）与通讯录同一操作（f08f21.png） | — |
| F09 | 已修复 | 「添加群聊」首屏即「创建群」页签表单（好友在册时直接呈现群名称+成员列表，f09b.png）；「收到的入群邀请」降为页签；空好友簿时「先去添加好友」CTA 引导（f09.png） | — |
| F10 | 已修复 | 建群表单无「trim」等内部术语；群名占位符「输入群名（1-64 个字）」为用户语言（f10.png） | — |
| F11 | 已修复 | 手动拨号弹窗为结构化分段（PeerId/地址/端口/传输，testid dial-peer/dial-port/dial-transport），含「从发现结果带入」与「从发现清单选择」选择器，语法提示保留（f11.png） | — |
| F12 | 已修复 | 发现行内快捷动作「拨号」「加为好友」齐备，跨卡全链贯通（详见下节专核，f12.png） | — |
| F13 | 已修复 | 中继地址两条均带「地址 1/地址 2」序号可视标签、占位符只放示例；引导地址弹窗字段标签「引导地址」；设置宣告/观测地址条目带「地址 1」标签（f13.png） | — |
| F14 | 未修复 | 设置 QUIC 端口输入 99999 后失焦：无任何即时提示，aria-invalid=null（f14.png） | 失焦无反馈。复现：设置 → QUIC 端口填 99999 → 失焦 → 页面无「端口需为 0-65535」，字段无 aria-invalid；错误仍推迟到保存时汇总（基线行为，非本条诉求） |
| F15 | 已修复 | rail 可见锚点含 #/messages（消息中心）与 #/docs（协议文档），直达入口常驻（f15.png） | — |
| F16 | 已修复 | 事件行尾显式「详情」按钮，点击后行内展开详情内容（页面文本增量确认）（f16.png） | — |
| F17 | 已修复 | UI 发邀请后聊天列表出现置灰占位条目「等｜等待对象丙｜等待对方同意」，主区提示「邀请已发出，等待对方同意；同意后即可开始私聊」（f17.png） | — |
| F18 | 已修复 | 连接后立即扫节点表全文：无「N 秒钟后」类未来时表述，「最后活跃」显示「刚刚」（f18.png） | — |
| F19 | 已修复 | 停止节点后状态卡显示「节点身份 DPCm2DVxsazo…」与「已停止，身份保留」，不再出现「节点身份 未知」（f19.png） | — |
| F20 | 已修复 | 事件筛选按域分组：连接/消息/群组/安全/节点 五组标题，chip 附命中计数（如「启动 2」），顶部「清除筛选」一键重置（f20.png） | — |
| F21 | 部分修复 | 发出的邀请卡：昵称「邀请对象乙」为标题，PeerId 缩略「Csb8yM…6zQ7」（span title 悬停可见完整 44 位），页面无 44 位全文（f08f21.png） | 卡内无复制按钮（span 仅 title 属性，无复制 affordance），不满足「缩略显示+复制按钮」的完整诉求 |
| F22 | 已修复 | 命令面板条目为「概览」无「仪表盘」；页头「概览」；palette 与页头同源（f22.png）。注：zh-CN.ts 仍残留 dashboard.title="仪表盘" 旧 key（页面未引用），建议顺手清理 | — |
| F23 | 部分修复 | 「邀请成员」选择器（group-invite-picker）有搜索框「搜索名称或 PeerId」，输入「成员三」即时过滤仅剩匹配项（f23.png）；建群表单内嵌成员列表当前无检索（r2/f23、r5、final 三轮实测一致） | 建群成员列表无搜索输入（inputs 仅群名称+成员 checkbox）；「已选区置顶」未见实现 |
| F24 | 已修复 | 添加好友空提交：「PeerId 不能为空」role=alert 可见，输入框 aria-invalid=true，aria-describedby=friend-add-peer-id-error 正确指向错误节点（f24.png） | — |
| F25 | 已修复 | 添加 Agent 弹窗主字段「WS 地址」默认展开可见，无需展开高级设置；「从发现清单选择节点（辅助填充）」+「选择节点」为辅助入口，自由录入路径保留（f25.png） | — |
| F26 | 已修复 | 顺序巡游 peers/settings/docs/messages 四路由：console「路由上报失败 invoke undefined」命中 0 条（f26.png） | — |
| F27 | 已修复 | 诊断页「日志文件为桌面端能力｜浏览器预览不运行节点，也没有本地日志…」说明性空态；最近错误折叠为「复制详情」，原始堆栈不直出（f27.png） | — |
| F28 | 已修复 | 经 #/chat?peer= 直达会话：输入 1950 字显示计数「1950/2000」；2100 字发送就地提示「消息不能超过 2000 字符」且内容保留（len=2100，未清空）（f28.png） | — |

## 特别核对：F12 跨卡协同全链（发现页快捷动作 → ?dial=/?add= 预填消费端）

全链贯通，逐跳证据：

1. 发现页局域网结果行内动作齐备：每行含指向 `/network/peers?dial=<target>` 与 `/contacts?add=<peerId>` 的快捷链接各一个。
2. 点「拨号」：href=`#/network/peers?dial=MJ1keroVRHf8…%40192.168.177.196%2Ft37834` → SPA 跳转后拨号弹窗自动打开，PeerId 段预填 `MJ1keroVRHf8…`、端口段预填 `37834`、传输段就绪（UX-E 消费端 peers-view dialParam + dial-compose 三段结构）。
3. 点「加为好友」：href=`#/contacts?add=tHi5qb94kV44EXQyNqPPFpUnDYDKGYy5cwfn7JvyDmXr` → 添加好友弹窗自动打开，PeerId 输入框值 `tHi5qb94kV44…` 预填（消费端 friend-section + chat-friend-add-dialog）。
4. 反向消费端亦实测可用：聊天空态「去通讯录」→ `#/contacts?add=`（自动弹添加好友弹窗）；`#/chat?peer=<id>` 直达会话（F28 使用）。

## 总体结论

**达到任务书验收标准。** 28 条增量 finding 中 25 条已修复、2 条部分修复（F21/F23，主诉求均落地、仅余按钮/局部检索缺口）、1 条未修复（F14，P2 打磨项）。P0 全清（F01-F05、F12 全部已修复），P1 除 F23 的建群列表子项外全清。跨卡协同（F12 → ?dial=/?add= 消费端）全链实测贯通。mock 态好友/群数据经页面内注入闭环，28 条全部有页面实测证据，无代码推断项。

## 遗留清单（建议派返工单）

1. **F14（未修复，P2）**：设置页数值字段失焦即时校验缺失。复现：`#/settings` → QUIC 端口填 99999 → 失焦 → 无「端口需为 0-65535」提示、无 aria-invalid（错误推迟到保存时汇总聚焦）。建议：数值/范围字段 onBlur 即时校验 + 就地 role=alert 提示。
2. **F21（部分修复，P2）**：消息中心「发出的」邀请卡缩略 PeerId（span title 悬停见全文）无复制按钮。建议：缩略节点旁补复制按钮（或点击缩略节点复制全文）。
3. **F23（部分修复，P2）**：建群表单内嵌成员列表无检索框（「邀请成员」选择器已修好）；「已选区置顶」未见。建议：复用 group-invite-picker 搜索框下沉到建群表单成员区。
4. **顺手项（非 28 条）**：a) zh-CN.ts `dashboard.title="仪表盘"` 旧 key 残留（页面已同源「概览」，建议清理防回潮）；b) 浏览器 mock 态 boot 时 `[data-watch]` 两条 console 报错（invoke/transformCallback undefined，应用已降级留日志，噪音为一次性非逐页，可考虑静默降级）。

## 走查方法附注（供复核）

- 每条 finding 的完整断言明细存 /tmp/final-walk/report.json（27 场景 × 每场景 2-6 条 check，含 DOM 文本证据）；截图逐场景落盘。
- 工具为 scripts/gui-agent.mjs 的 /tmp 改造副本（端口 9235 专属、flow 批量场景、Chrome 加 --no-proxy-server 规避系统代理劫持 127.0.0.1）。仓库零改动。
- 已知走查口径：F10 的「1-64 个字」承载在占位符（innerText 不含 placeholder），以 placeholder 实测值为准；F13 的「宣告地址添加弹窗」子项因脚本点击时序未取到弹窗内输入框，但页面既有条目「地址 1」标签已直接可见，判定不受影响。