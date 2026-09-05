# GUI 外壳重设计：信息架构设计（icon rail 四入口）

状态：v1（2026-09-05，设计会话制定，按已拍板决策展开，本文即施工规格）；既有壳规划见 [gui-plan.md](gui-plan.md)，菜单登记见 `apps/gui/src/config/menu.def.ts`。

拍板基线（不再开放讨论）：左侧收敛为窄图标栏（icon rail），一级入口 4 个——聊天、通讯录、网络、设置（沉底）；单聊/群聊/agent 会话统一为「聊天」一种心智，聊天是唯一主入口；旧路由全部保留重定向；快捷键重映射到 4 入口；命令面板仍可达全部子页/tab；表单三律（前置校验同口径、禁自由文本、历史值下拉）已拍板，落点见 2.5 与 3.4。

事实基线（现状代码，施工对照用）：现壳为 10 个平级文字菜单（`MENU_ENTRIES`，路径 `/` `/peers` `/discovery` `/relay` `/chat` `/group` `/acp` `/events` `/settings` `/diagnostics`）；hash router（`createHashRouter`）；快捷键 Cmd/Ctrl+1..9 按注册序跳前九个路由、Cmd/Ctrl+K 命令面板；窗口最小 960x600，断点 md 768 / lg 1024 / xl 1280；侧栏 w-60（折叠 w-14）、顶栏 h-14、底部状态栏 h-8；全代码库当前无未读（unread）状态。

## 一、信息架构总览

### 1.1 rail 规格与四入口

- rail：常驻 w-14 窄图标栏（沿用现折叠宽度），仅图标 + tooltip + 选中态高亮，不再提供折叠形态（rail 本身即最窄形态）。
- 顶栏（节点状态 pill、启停、主题/语言）与底部状态栏（运行状态、监听端口、活跃连接、版本）保留不动。
- rail 自上而下：聊天、通讯录、网络；弹性空隙后底部固定：设置。与使用频率分层一致——聊天每日高频居首，设置低频沉底。

| rail 入口 | 路由 | 图标（lucide） | 承载内容 |
|---|---|---|---|
| 聊天 | `/chat` | MessageCircle（沿用） | 单聊 + 群聊 + agent 会话，双栏 |
| 通讯录 | `/contacts` | UsersRound（沿用群聊图标） | 好友/群/agent 三区 + 添加流 + agent 详情 |
| 网络 | `/network` | Network（沿用） | 概览/节点/发现/中继/事件/诊断 六 tab |
| 设置 | `/settings` | Settings（沿用） | 原设置页原样 |

### 1.2 旧 10 页 → 新位置映射

| 旧菜单（旧路由） | 去向 | 形态 |
|---|---|---|
| 聊天（`/chat`） | `/chat` | 就地升级为双栏，吸收 `/group` `/acp` 会话 |
| 群聊（`/group`） | `/chat`（群条目混排）+ `/contacts` 群区（管理） | 页面拆解，组件迁移不复制 |
| ACP（`/acp`） | `/chat`（agent 会话混排）+ `/contacts` agent 区（endpoint/权限管理） | 同上 |
| 仪表盘（`/`） | `/network/overview` | 内容拆吸收进概览 tab |
| 节点（`/peers`） | `/network/peers` | 视图组件整体迁移为 tab |
| 发现（`/discovery`） | `/network/discovery` | 同上 |
| 中继（`/relay`） | `/network/relay` | 同上 |
| 事件（`/events`） | `/network/events` | 同上 |
| 诊断（`/diagnostics`） | `/network/diagnostics` | 同上 |
| 设置（`/settings`） | `/settings` | 原样，rail 沉底入口 |

原则：五类排障面视图组件 `git mv` 整体迁移复用（不复制、不留双源）；群聊/ACP 页拆解为「会话流进聊天、管理与添加进通讯录」两半。

## 二、聊天页详设（/chat）

### 2.1 双栏布局与响应式收窄

- 结构：左侧通栏会话列表（含顶部搜索框），右侧会话记录（消息流 + composer），列表与记录间以现分割线样式分栏。
- 宽度策略（窗口最小 960x600 约束下）：
  - ≥1280（xl）：列表 320px 固定，记录区 flex-1。
  - 960–1279：列表 264px。
  - <768（防御性规则，桌面壳理论不可达但保留）：单栏互斥模式——默认显列表，选中会话切入记录，记录区左上返回按钮回列表。
- 选中态路由化：`/chat?peer=<peerId>`、`/chat?group=<groupId>`、`/chat?agent=<endpointId>`；无 query 时不选中，右侧显示空态（logo + 「选择或发起会话」）。刷新/深链直落选中会话。
- 会话记录区复用现 `message-list`/`message-bubble`/`composer`（1:1 与群）与 ACP `transcript`/`prompt-composer`（agent），不重写消息渲染。

### 2.2 会话条目统一模型（字段级对齐）

会话列表把三来源统一为一种条目形状 `ConversationEntry` 混排（store 层聚合，渲染层无来源分支）：

| 统一字段 | 单聊来源 | 群聊来源 | agent 来源 |
|---|---|---|---|
| `id` | `ChatFriendJson.peerId`（base58） | `GroupJson.groupId`（UUID） | `endpointId`（本地生成 UUID，新引入，替代 wsUrl 作主键） |
| `kind` | `"friend"` | `"group"` | `"agent"` |
| `title` | `nickname`（空回退 peerId 缩略，沿用现规则） | `name` | endpoint 别名（本地起名，默认 wsUrl host:port） |
| `subtitle` | `note`，无则省略 | 「N 成员」 | 连接态文案（已连接/连接中/已断开） |
| `kindMark` | 头像首字，无角标 | 头像首字 + 群角标 | Bot 图标 + 连接态色点（绿/黄/红） |
| `statusBadge` | 无 | `state≠active` 时徽标（left=secondary / kicked=destructive / disbanded=outline，沿用现映射） | `error` 时 destructive 徽标（断连） |
| `lastPreview` | `lastMessageByPeer` 按预览规则 | 群最后一条消息按预览规则，前缀发送者昵称（自己显「我」） | transcript 最后一条 user/agent 文本截断 40 字；无则显连接态 |
| `lastTsMs` | 最后一条消息 `tsMs`，无消息用好友创建序 | 群最后一条消息 `tsMs`，无消息用 `GroupJson.tsMs` | 最后一次交互时间（新记录，acp-store 落地） |
| `unread` | 见 2.3 | 见 2.3 | 见 2.3 |
| `sendState` | 最近一条本端消息 `status`（pending/sent/delivered/failed） | 最近一条本端消息 `status`（pending/delivered/failed） | 仅 pending（等待 agent 响应）与 error（连接失败）两态 |

预览规则（`lastPreview`，按消息 `kind`）：`text`→正文单行截断；`image`→「[图片]」；`audio`→「[语音]」；`video`→「[视频]」；`file`→「[文件] 」+ `media.name`。（沿用现 `summaryOf` 语义并补齐音视频文案。）

条目上发送状态呈现：仅当最近一条为本端消息且 `failed` 时，条目右侧显红色感叹角标（点击进会话并定位该消息，复用 `use-retry-send`）；`pending` 显时钟图标；其余不显示。

排序：`lastTsMs` 降序，无消息条目按加入时间降序排在有消息之后；`statusBadge` 非空的条目不特殊排序（状态靠徽标传达，不打乱时间心智）。置顶/折叠分组均不引入。

### 2.3 未读计数（新增状态）

现状全库无 unread，本节为新能力规格：

- 计数来源：本端未选中该会话时收到消息——1:1 `chat_message` 事件、群 `chat_group_message` 事件、agent 会话连接期收到完整回复——对应条目 `unread+1`。
- 清零：选中会话（query 落定）即清零；自己发出的消息不计入。
- 持久化：仅内存态（store），不落盘；重启归零。理由：离线推送语义尚不存在，落盘未读反而制造假信号。
- 呈现：条目右侧灰色圆点数字，≥100 显「99+」；rail 聊天图标显合计角标（三来源求和）。
- agent 例外：endpoint 断开期间不计未读（无推送语义），恢复连接后从当次会话起计。

### 2.4 会话搜索

- 位置：列表顶部常驻搜索框；行为为列表内即时过滤（无独立搜索页、无历史全文检索——历史检索明确为非目标）。
- 匹配范围（对 `ConversationEntry`）：`title` 与 `subtitle` 不区分大小写子串；`peerId`/`groupId` 前缀匹配；agent 额外匹配 wsUrl host。不做命中高亮。
- 过滤只影响显示不动排序；清空恢复。无结果显示「无匹配会话」空态。

### 2.5 表单规范（三律总则）

表单三律（已拍板，全应用表单适用；聊天页落点在本节，通讯录落点见 3.4）：

1. **前置校验**：有约束的字段必须有前置校验，口径与后端一致（同格式/同上限/同错误语义），错误码稳定并经 i18n 渲染（原始错误串只进提示详情，不当正文直出）。
2. **禁自由文本**：能用下拉/选择器的场景禁止自由文本输入——候选集来自既有数据（分组、endpoint、好友等），仅「新建 X」路径允许输入。
3. **历史值下拉**：有历史值的输入提供历史值下拉——聚焦时列出该字段历史提交值（去重、最近在前）。

聊天页表单面 = composer（文本/媒体）与失败重发，落点：

- 空文本禁发（Enter 仅在有内容时发送，沿用现 composer 行为）；媒体发送前置校验 mime 白名单与 ≤64MiB 上限，口径与后端 `ChatMediaInput` 一致，超限本地前置拦截并 i18n 提示，不发无效请求。
- composer 无下拉/历史值场景，三律 (2)(3) 显式不适用（防施工者误加）。
- 验收归 P1（见六）。

## 三、通讯录页详设（/contacts）

### 3.1 三区布局

单页纵向三分节，节间分割线，页顶锚点条（好友 | 群 | Agent，点击滚动定位，当前节高亮）：

- **好友区**：行 = 头像 + 昵称 + 备注 + 分组名 + 在线状态点（复用 `peer-status`）；行内操作：发消息（跳 `/chat?peer=`）、移动分组（迁移 `chat-friend-move-dialog`）、删除（迁移 `chat-friend-remove-dialog`，危险确认）。分组折叠交互随行迁移，不再是页面级结构。
- **群区**：行 = 群名 + 成员数 + 我的角色（owner/成员）+ 状态徽标（沿用四态映射）；行内操作：发消息（跳 `/chat?group=`）、邀请成员、退群（危险确认）。left/kicked/disbanded 群置底保留（数据不删仅置位的语义沿用）。
- **Agent 区**：行 = 别名 + wsUrl host + 连接态 + 权限档摘要；行内操作：发消息（跳 `/chat?agent=`）、详情（见 3.3）、停用/删除。

每节标题右侧常驻「添加」按钮；空态沿用 `EmptyState` 并带添加引导。

### 3.2 三种添加流

**加好友（迁移现有邀请制流）**：

1. 好友区「添加」→ `chat-friend-add-dialog`（迁移）：填对方 PeerId + 备注 + 分组。
2. 提交生成 out 邀请（`direction=out`），好友区顶部出现「待对方同意」条目，可撤回（`cancelInvite`）。
3. 收到 in 邀请：通讯录页顶显红点徽标（rail 通讯录图标同步角标），展开待处理列表，逐条 接受（填昵称）/拒绝。

**入群/邀请（迁移群聊页流）**：

1. 群区「添加」→ 二选一弹框：创建群（填群名，成为 owner）／处理收到的入群邀请。
2. owner 在群行「邀请成员」→ 从好友多选发出邀请；受邀方在其群区看到待处理条目，接受后入群（`GroupJson.members` ≤32 上限校验沿用）。

**配置 agent endpoint（迁移 ACP config-panel 流）**：

1. Agent 区「添加」→ 表单：wsUrl（默认 `ws://127.0.0.1:8787` 沿用）、token（可空）、可选 peer、别名。
2. 「测试连接」先行验证；未通过测试的 endpoint 允许保存但显警告徽标。
3. 保存进入收藏列表（沿用 localStorage 存档语义），保存后即可发起会话。

### 3.3 agent 管理详情页

行点击「详情」打开右滑抽屉（对齐 gui-plan 检查器抽屉心智），内容五块：

1. **连接**：endpoint 三字段（wsUrl/token/peer）只读展示 + 编辑态；「测试连接」；连接态与最近错误（失败路径显式呈现，不静默）。
2. **能力**：迁移 `capabilities-card`（agent 声明的能力清单）。
3. **权限策略**：迁移 `permission-grading` 配置面板——按操作分级（自动允许/每次询问/拒绝）的默认档与例外规则；变更即时生效于后续会话。
4. **会话**：该 endpoint 的历史会话列表，点击跳 `/chat?agent=` 并载入对应 transcript。
5. **危险区**：停用（保留配置、断开且不可发起）/ 删除（AlertDialog 二次确认，含「将同时移除本地会话记录索引」影响说明）。

### 3.4 表单规范

三律定义见 2.5。通讯录各表单落点与现状标记：

| 表单 | 三律落点 | 现状 |
|---|---|---|
| 加好友（PeerId/备注/分组） | (1) PeerId base58 格式、备注 trim ≤64 与后端同口径，错误码稳定经 i18n | 已完备，保持不退化 |
| 创建群（群名） | (1) 群名 trim 1..=64 与后端同口径 | 已完备，保持不退化 |
| 移动分组 | (2) 已存在分组必须下拉选择（候选 = 现 `orderedGroups` 清单），仅「新建分组」路径才出现文本输入 | **需改造**（现为自由文本） |
| agent endpoint（wsUrl/token/peer/别名） | (1) wsUrl 做 URL 格式校验，口径稳定经 i18n；(3) wsUrl 聚焦下拉列出历史保存值（saved endpoints 去重、最近在前） | **需改造**（补历史值下拉与稳定错误码） |
| 资料编辑（昵称等） | (1) 昵称 trim ≤64 同口径 | 已完备，保持不退化 |
| 设置项 | (1) 各配置字段校验口径不变 | 已完备，保持不退化 |

改造项验收归 P2：移动分组表单显「选择分组」下拉 + 「新建分组…」入口（选中才展开输入框）断言；endpoint 表单历史值下拉断言；表单错误路径全部走稳定错误码 + i18n key 断言。

## 四、网络页详设（/network）

### 4.1 tab 划分与旧页映射

子路由即 tab（利于重定向与深链），容器内渲染 tab 条，`/network` 索引重定向 `/network/overview`：

| tab（子路由） | 旧页 | 内容 |
|---|---|---|
| 概览 `/network/overview` | `/` 仪表盘 | 见 4.2 |
| 节点 `/network/peers` | `/peers` | 已知节点表 + 拨号/ping/复制 PeerId + 检查器逐跳抽屉，组件原样迁移 |
| 发现 `/network/discovery` | `/discovery` | mDNS 开关与结果、rendezvous 地址簿增删与手动触发 |
| 中继 `/network/relay` | `/relay` | relay 地址配置、会话水位、逐跳成败统计 |
| 事件 `/network/events` | `/events` | 实时事件流过滤/搜索/暂停/清空/导出 |
| 诊断 `/network/diagnostics` | `/diagnostics` | 错误缓冲 + 日志路径 + 尾部 50 行（5s 自刷）+ 一键清理 |

tab 记忆：当前 tab 会话级保持，重进 `/network` 无子路由时落 overview；tab 条为 `NETWORK_TABS` 局部登记数组（append-only 注释约束，不进中央 menu.def）。

### 4.2 概览页信息构成

吸收现仪表盘全部信息卡，按「三秒看清 + 一跳排障」组织：

1. 节点状态卡：运行状态、PeerId（复制）、监听地址、运行时长、启停按钮（保留——高频运维就地可达）。
2. 指标卡：发现、连接、中继会话、门禁拒绝四指标（数据源 `metrics_get` 不变）。
3. 拨号跳成功率（DialHop 聚合）。
4. 最近事件 5 条（只读摘要），标题栏「查看全部」跳 `/network/events`。
5. 排障入口行：三个链接直达 节点/中继/诊断 tab（「排障一跳直达」的落点）。

## 五、导航与可达性

### 5.1 快捷键方案（重映射）

| 快捷键 | 现行为 | 新行为 |
|---|---|---|
| Cmd/Ctrl+1 | `/` 仪表盘 | `/chat` |
| Cmd/Ctrl+2 | `/peers` | `/contacts` |
| Cmd/Ctrl+3 | `/discovery` | `/network` |
| Cmd/Ctrl+4 | `/relay` | `/settings` |
| Cmd/Ctrl+5..9 | 依次旧页 | 不注册（保留给未来 rail 扩展，不占用） |
| Cmd/Ctrl+K | 命令面板 | 不变 |
| Esc | 关浮层 | 不变 |

实现沿用 `use-hotkeys.ts` 数字→`MENU_ENTRIES` 序机制（rail 入口即新的注册序），对话框打开退避逻辑不变。

### 5.2 命令面板覆盖

面板分组与条目清单（施工即清单）：

- 「导航」组：4 个 rail 入口 + 网络 6 tab（`/network/overview` 等）+ 通讯录 3 区锚点（好友/群/Agent）——共 13 项，覆盖全部子页/tab。
- 「会话」组：最近 8 个会话条目（复用 2.2 统一模型，取 `lastTsMs` 前 8）。
- 「动作」组：加好友、创建群、添加 agent endpoint、拨号节点（带输入）、复制本机 PeerId。
- 「节点」组：现 peer 列表与地址复制项保留。

约束：面板数据源与 rail/`MENU_ENTRIES` 解耦——面板持有独立全量注册表，rail 收敛不得减少面板项；验收含「13 个导航项逐一可达」断言。

### 5.3 旧路由重定向表

| 旧路由 | 重定向目标 | 说明 |
|---|---|---|
| `/` | `/network/overview` | 仪表盘整体吸收 |
| `/peers` `/discovery` `/relay` `/events` `/diagnostics` | `/network/<同名>` | 五排障面一一直达 |
| `/group` | `/chat?kind=group` | 落聊天并聚焦排序最前的群条目；无群则空态（待拍板项 1） |
| `/acp` | `/chat?kind=agent` | 同上，agent 条目 |
| `/chat` `/settings` | 不变 | 路由保留 |
| `/network` | `/network/overview` | 索引归一 |

实现要点：redirect 层为常驻中间路由（非迁移期临时物），在路由树集中注册；带 query 深链（如 `/chat?peer=x`）参数原样透传；hash router 下用路由树内 loader/`<Navigate>` 重定向，不引入全页刷新。重定向表配套路由测试（每行一条断言，见 P0 验收）。

## 六、分期实施计划

四期，每期独立可验收、可单独 revert。串行约束先行说明：`menu.def.ts`、`App.tsx` 路由树、i18n types+locale 为中央登记文件，各期对它们的注册改动各压独立小提交（append-only 纪律沿用）；实现工作可并行的期，其登记提交合并进 main 天然串行（ff-only），i18n 冲突一律在 feature 侧消化。

| 期次 | 内容 | 可并行性 |
|---|---|---|
| P0 外壳骨架 | rail 四入口替换文字侧栏；路由树重构（`/network/*` 子路由先直接挂旧视图组件、`/contacts` 占位页）；5.3 重定向全量落地；5.1 快捷键与 5.2 面板注册表更新 | 先行，其余全部依赖 |
| P1 聚合聊天 | `/chat` 双栏 + `ConversationEntry` 统一模型 + 群/agent 混排 + 未读 + 搜索 + 2.1 响应式 | P0 后可与 P3 并行 |
| P2 聚合通讯录 | 三区 + 三添加流迁移 + agent 详情抽屉 | 依赖 P1（复用统一条目模型、连接态与未读语义） |
| P3 聚合网络 | `/network` tab 化容器 + 概览页组装 + `/` 旧仪表盘组件退役 | P0 后可与 P1 并行 |

依赖逻辑：P1 与 P3 无共享实现文件（views/chat 对 views/network），仅中央登记文件各自小提交；P2 排后因通讯录行模型直接 import 聊天期的条目/状态类型。

每期通用门禁：`pnpm -C apps/gui build` 零错误 + 定向 vitest 绿 + 期末 `make check` 全绿。各期影响面与验收要点草案：

- **P0**：影响 `app-layout.tsx`、`menu.def.ts`（独立小提交）、`App.tsx`、`use-hotkeys.ts`、`command-palette.tsx`、i18n。验收：5.3 重定向表每行一条路由断言（含带 query 透传）；rail 恰 4 入口且设置在底；Cmd/Ctrl+1..4 四跳断言；面板 13 导航项逐一可达断言；旧 10 视图内容均可在新位置打开（沿用现启动冒烟机制）。
- **P1**：影响 `views/chat/**`、`components/chat/conversation-list.tsx`（重写）、`stores/chat-store.ts`（unread/聚合）、`stores/group-store.ts`、`acp/acp-store.ts`（会话摘要与最后交互时间）、i18n。验收：三来源条目同列混排渲染测试；预览规则 5 种 kind 断言；未读加一/选中清零/rail 合计角标断言；搜索过滤（标题/备注/ID 前缀）断言；`?peer=`/`?group=`/`?agent=` 深链选中断言；<768 单栏互斥切换断言（jsdom 视口模拟）；composer 前置校验断言（空文本禁发、媒体 mime/≤64MiB 本地拦截且 i18n 提示，2.5）。
- **P2**：影响 `views/contacts/**`（新）、迁移 `chat-friend-add-dialog` 等 4 个对话框、`acp/config-panel.tsx`、`permission-panel.tsx`、`capabilities-card.tsx`、i18n。验收：三区空态与行操作断言；加好友 out/in 邀请全流程测试（沿用现 chat-friend-add 测试迁移）；创建群→邀请→入群流程断言；endpoint 添加→测试连接→保存→发起会话断言；详情抽屉五块渲染与权限档变更生效断言；表单三律改造项断言（移动分组下拉 + 新建分组路径、endpoint 历史值下拉、错误码 i18n 快照，3.4）。
- **P3**：影响 `views/network/**`（新容器）、五视图 `git mv` 迁移、`views/dashboard` 退役、i18n。验收：6 tab 切换与子路由直达断言；概览页四组信息卡 + 5 条最近事件断言（复用现 dashboard 测试改造）；「查看全部」跳转断言；`grep -r views/dashboard` 为空确认无残留引用。

预估工作量比：P0 ≈ P1 > P2 > P3。每期完成即合并（短命分支纪律），不等四期齐。

## 七、风险与回滚

**迁移期新旧并存策略**：

- 并存单元是「路由」而非「页面」：P0 起旧视图组件挂在新路由下，旧 URL 经常驻 redirect 层到达同一组件——任何时刻只有一份组件实现，无双源漂移。
- P0 后旧文字侧栏即移除，不存在新旧菜单同时可见的混淆期；用户侧唯一可见过渡是 URL 变化，由 redirect 兜底。
- 群聊/ACP 页在 P1 前保持整页形态挂新壳内（P0 占位策略），P1/P2 拆解后旧页路由仅剩 redirect。

**风险与对策**：

| 风险 | 对策 |
|---|---|
| redirect 丢 query 参数断深链 | 5.3 验收含带参用例；redirect 统一走一个透传函数，禁止各处手写 |
| 未读状态侵入 chat-store 触发事件回放回归 | unread 为独立 reducer 切面，现有 `chat-store.events.test` 全量保留并扩未读用例 |
| i18n key 大迁移漏译 | 旧 key（`peers.*` `events.*` 等）随组件迁移原样复用，仅新增 key 走新命名空间（`contacts.*` `network.*`）；类型安全 key 机制兜底编译期漏报 |
| 网络页容器与六视图合写超文件红线（300 行） | tab 容器、tab 条、概览页分文件；五视图 `git mv` 后本就不超限 |
| 并行期（P1∥P3）i18n/路由树冲突 | 登记文件改动独立小提交 + 合并前 worktree 内 `git merge main` 反向同步，冲突在 feature 侧消化 |
| rail 收敛后低频功能（诊断）被发现性下降 | 命令面板全量覆盖 + 概览页排障入口行兜底（4.2 第 5 块） |

**回滚**：每期独立提交序列，可单独 `git revert`；`menu.def.ts` 独立小提交使 rail 回滚不拖累页面实现；redirect 表变更独立提交，回滚任一期不影响其余期已落地的深链。最坏情况（整壳回退）：revert P0 合并提交即回到 10 菜单壳，redirect 层随 P0 一并消失，旧路由恢复原义。

## 待拍板项（默认按推荐方案施工，拍板后替换）

1. **`/group`、`/acp` 深链落地形态**。推荐：`/chat?kind=group|agent` 聚焦排序最前的对应条目（5.3 已按此写）。理由：旧页无会话参数语义，聚焦第一条比空列表更可达；若拍板改「不带 query 落 `/chat`」仅需改 redirect 表一行。
2. **未读计数持久化**。推荐：仅内存态（2.3 已按此写）。理由：无离线推送语义，落盘未读会显示陈旧假信号；待 agent 离线队列语义出现后再升级。
3. **agent 条目 `lastPreview`**。推荐：transcript 最后一条 user/agent 文本截断（2.2 已按此写）。理由：与单聊/群「最后一条消息」心智对齐；备选「仅连接态文案」实现更省但信息量低。
4. **通讯录三区布局形态**。推荐：纵向三分节 + 锚点条（3.1 已按此写）。理由：960px 窗口下限内三栏或内部 tab 都会挤压行操作；若拍板改「内部三 tab」，3.1 锚点条换 tab 条，其余规格不变。
5. **概览页是否保留节点启停按钮**。推荐：保留在概览（4.2 已按此写）。理由：启停属高频运维且顶栏 pill 已提供状态上下文；若拍板「收进节点 tab」，删 4.2 第 1 块按钮、顶栏 pill 点击跳节点 tab 即可。
