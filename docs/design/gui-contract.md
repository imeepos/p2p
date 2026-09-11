# GUI 前后端契约 v1（冻结）

状态：2026-09-02 协调会话冻结。Rust 侧（A）与前端（B/C/D）各自对本契约编程，互不等待。
改动须经协调会话裁决：只允许"新增字段/新增命令"的加法，禁止改已有形状。

## 1. Tauri 命令表（invoke）

所有命令在 `apps/gui/src-tauri` 注册；参数/返回一律 JSON（camelCase）。Err 一律返回可读中文错误串。

| 命令 | 参数 | 返回 | 语义 |
|---|---|---|---|
| node_start | cfg: GuiConfig | NodeStatus | 构建 Node（Node::builder）并启动；已运行则 Err |
| node_stop | - | NodeStatus | node.shutdown()；未运行也返回当前 status（幂等） |
| node_status | - | NodeStatus | 本地/连接数/运行时长/监听地址快照 |
| metrics_get | - | MetricsJson | node.metrics() 映射 |
| config_get | - | GuiConfig | 读持久化配置（无文件返回默认值） |
| config_save | cfg: GuiConfig | GuiConfig | 原子写盘；不改变运行中节点 |
| peer_dial | target: string | DialReport | add_peer_address + node.connect，回收期间 DialHop 事件为逐跳报告 |
| peer_connect | peerId: string | DialReport | node.connect（地址簿直连，已知节点免重复登记），逐跳报告同 peer_dial；v4 加法新增 |
| peer_disconnect | peerId: string | boolean（wasConnected） | 出池并关闭该 peer 连接；幂等，未在册连接返回 false；PeerDisconnected 事件照常发出；v4 加法新增 |
| peer_ping | peerId: string, timeoutMs: number | PingOutcome | 复用 echo 协议 node.request（同 CLI ping），返回 rtt 与期间逐跳 |
| identity_reset | confirm: boolean | NodeStatus | 危险：停止节点并删除身份数据目录内种子文件（必须 confirm=true），返回重置后的状态（未运行） |
| metrics_history | - | MetricsPoint[] | 后端每 5s 采样最近 120 点（10 分钟窗口），供仪表盘趋势图；v2 加法新增 |
| frontend_log_append | lines: string[] | void | 前端错误 JSONL 批量追加到 app_log_dir/frontend.log（超 1MB 轮转 frontend.log.1）；v3 加法新增（G-H 观测） |
| frontend_log_tail | maxLines?: number | string[] | 读 frontend.log 末尾 maxLines 行（默认 200，上限 1000）；v3 加法新增（G-H 观测） |
| frontend_log_path | - | string | frontend.log 绝对路径（诊断页展示 + 外部 Agent 定位）；v3 加法新增（G-H 观测） |
| update_check | - | UpdateCheckResult | 查询 GitHub 最新稳定 release 并与当前版本比较；无候选时 latestVersion 为 null；网络/解析失败返回 Err；v4 加法新增（G-U1） |
| update_open_release_page | url: string | void | 系统浏览器打开更新页；url 必须 https 且 host 为 github.com，白名单外 Err；v4 加法新增（G-U1） |
| profile_get | - | NodeProfile | 读持久化节点资料，无文件返回默认值（全空）；v6 加法新增（§11） |
| profile_save | profile: NodeProfile | NodeProfile | 校验（长度/头像格式）后原子写盘；不改变运行中节点，无需重启即生效；v6 加法新增（§11） |

## 2. 事件通道

后端 `app.emit("node-event", <NodeEventJson>)`；前端 `listen("node-event", ...)` 单例订阅入 store。

NodeEventJson 判别联合（type 字段）；所有变体均可携带可选 `tsMs?: number`（后端发射时刻毫秒时间戳，前端缺省时以本地接收时间兜底，加法字段不破坏既有实现）。

`peer_discovered` 自 v5 起携带必填 `source: "mdns" | "rendezvous" | "manual"`（加法字段，语义见 §10）：

```ts
| { type: "peer_discovered"; peer: string; addrs: string[] }
| { type: "peer_connected"; peer: string }
| { type: "peer_disconnected"; peer: string }
| { type: "listen_failed"; addr: string; reason: string }
| { type: "dial_failed"; peer: string | null; reason: string }
| { type: "protocol_violation"; peer: string; reason: string }
| { type: "dial_hop"; peer: string; hop: "direct" | "punch" | "relay"; ok: boolean; detail: string }
| { type: "node_started"; listenAddrs: string[] }
| { type: "node_stopped" }
| { type: "node_error"; reason: string }
```

## 3. 数据类型

```ts
interface GuiConfig {
  quicPort: number;            // 0 = 随机
  tcpPort: number;             // 0 = 随机
  enableMdns: boolean;
  dataDir: string;             // 默认 app 数据目录下 p2p-data
  bootstrap: string[];         // 出厂内置两个公网 rendezvous："43.240.223.138/u3400"、"121.196.193.177/u3400"（可编辑）
  relayAddrs: string[];        // 出厂内置两个公网 relay："43.240.223.138/u3403"、"121.196.193.177/u3403"（可编辑）
  advertisedAddrs: string[];
  observationPort: number | null;
  observationAddrs: string[];  // 观测反射端点（socket 语法 ip:port），如 "121.196.193.177:3402"
}

// 空列表语义：bootstrap/relayAddrs/observationAddrs 为空时，节点装配回落
// 出厂默认端点（state.rs with_factory_fallback），持久层不回写。

interface NodeStatus {
  running: boolean;
  peerId: string | null;       // base58(sha256(pubkey))
  listenAddrs: string[];
  uptimeSecs: number;
  startedAtMs: number | null;
  config: GuiConfig;           // 运行中节点的生效配置；未运行回持久化配置
}

interface MetricsJson {
  dialDirectOk: number;  dialDirectFail: number;
  dialPunchOk: number;   dialPunchFail: number;
  dialRelayOk: number;   dialRelayFail: number;
  addrDialFailures: number;
  relayReconnects: number;
  gateDenialsTotal: number;
  activeConnections: number;
  relaySessionsActive: number;
}

interface DialHopJson { hop: "direct" | "punch" | "relay"; ok: boolean; detail: string }
interface DialReport { peer: string; hops: DialHopJson[]; ok: boolean; totalMs: number }
interface PingOutcome { ok: boolean; rttMs: number | null; hops: DialHopJson[]; error: string | null }
interface MetricsPoint { tMs: number; activeConnections: number; relaySessionsActive: number; dialOkTotal: number; dialFailTotal: number }
```

## 4. tauri.conf.json 关键约定（A 侧遵守，B 侧依赖）

- productName: `p2p-console`；identifier: `com.p2p.console`；
- build.beforeDevCommand: `pnpm dev`；devUrl: `http://localhost:5173`；
  beforeBuildCommand: `pnpm build`；frontendDist: `../dist`；
- 窗口：标题 p2p-console，宽高 1280x800， minWidth 960 / minHeight 600。
- Rust 依赖仅 path 引用 `../../../crates/p2p`（含传递依赖）；src-tauri/Cargo.toml 声明
  `[workspace]` 空表以脱离根 workspace（根已 exclude，双保险）。

## 5. 前端服务层（B 侧交付，C/D 消费）

- `src/lib/ipc.ts`：上述命令的类型化封装（泛型 invoke），唯一 IPC 出口。
- `src/lib/mock-ipc.ts`：同签名的 mock 实现（模拟发现/连接/事件序列），`VITE_MOCK_IPC=1` 时 ipc.ts 内部切换；
  视图层对真实/mock 零感知。
- `src/stores/node-store.ts`（zustand）：status/metrics/peers/events 状态 + 订阅 node-event 单例。

## 6. target 与地址语法（澄清）

- peer_dial 的 target 格式：`<peer_id>@<addr>`，如 `3xY9...ab@192.168.1.5/3400`；
  addr 语法与 bootstrap/relay 一致：`ip/u端口` = QUIC，`ip/t端口` = TCP（对齐 README 与 TransportAddr）。
- 解析失败（缺 @、peer_id 非 base58、addr 非法）返回 Err，不静默。

## 7. 验收对齐点

- A：serde 序列化字段名与上表逐字一致（camelCase，含 Option 序列化为 null）；契约单测覆盖全部类型 roundtrip。
- B：ipc.ts 的 TS 类型与上表逐字一致；mock 与真实实现同签名。
- 两边都不得私自改名；发现契约缺口 → 报协调会话，走加法修订。
## 8. 前端错误落盘（v3 加法，G-H 观测）

前端 `src/lib/error-report.ts` 采集 window error / unhandledrejection / console.error，
序列化为 JSONL（字段 ts/kind/message/stack）批量调 frontend_log_append 落盘：

- 路径：`app_log_dir()/frontend.log`（macOS 即 `~/Library/Logs/com.p2p.console/frontend.log`）；
  超 1MB 轮转为 frontend.log.1（单代覆盖），tail 上限 1000 行。
- 浏览器/mock 模式（无 Tauri）：降级写 localStorage 键 `p2p-console.frontend-log`；
  mock 诊断后端（mock-diagnostics.ts）保留同签名实现，仅供测试内使用——
  运行时诊断固定走 Tauri IPC，诊断页禁止展示 mock 数据（2026-09-03 裁决）。
- 感知通道语义：外部进程（Agent/运维）直接读文件即可掌握前端错误，无需打开 DevTools。

## 9. 在线更新检查（v4 加法，G-U1/G-U2）

- 数据源：`https://api.github.com/repos/imeepos/p2p/releases?per_page=10`（公开只读接口；
  请求必须带自定义 User-Agent，GitHub 拒绝无 UA 请求）。
- 候选过滤：仅取 `draft=false` 且 `prerelease=false` 且 tag_name 为三段语义版本（容忍
  `client-v` / `v` 前缀与裸三段三种形态）的最新一条；无满足条件的条目时 latestVersion 为 null。
- 版本比较：逐段数值比较（0.10.0 > 0.9.0），禁止字符串比较；hasUpdate = latest > current。
- UpdateCheckResult：

```ts
interface UpdateCheckResult {
  currentVersion: string;        // 应用当前版本（tauri.conf version）
  latestVersion: string | null;  // 无候选时 null
  hasUpdate: boolean;
  releaseUrl: string | null;     // release html_url
  releaseName: string | null;
  releaseNotesMd: string | null; // release body 原文
  publishedAtMs: number | null;
  checkedAtMs: number;
}
```

- 失败语义：网络失败 / 响应非法 / 版本解析失败一律返回 Err（可读中文）并留日志，禁止静默吞。
- 无状态：后端不缓存不轮询；轮询节奏由前端驱动（启动后 + 定期 + 手动），无新增事件通道。
- HTTP 超时 10s；端点为编译期常量，不做用户配置。
- 本节只覆盖检查与提醒；程序内下载/安装/重启闭环见 §13（v8 加法）。

## 10. 邻居来源字段 source（v5 加法，2026-09-03 邻居表复盘）

`peer_discovered` 事件新增必填 `source` 字段（`"mdns" | "rendezvous" | "manual"`），
值来自 swarm 地址簿的按端聚合来源，取覆盖面最强档：mdns > rendezvous > manual。

- 语义：来源是"地址知识从哪来"，与连通性无关；对已发现节点手动拨号不会改变其来源
  （manual 地址仅在拨号对话框等显式登记路径产生）。
- 前端消费：邻居表来源列直读该字段，废除"有 dial_hop 记录即视为手动"的推断。
- 前端活跃度语义（展示层约定，随本修订一并生效）：`lastSeenMs`（最后活跃）只由正向
  证据刷新——发现源（mdns/rendezvous）的 peer_discovered 与 peer_connected；manual 来源
  的 peer_discovered（本端自身登记）、dial_hop（成败均可能出现）、peer_disconnected
  （可能来自发现缓存 TTL 过期）均不刷新。已发现/离线徽标据此推导，拨号失败不再把
  死节点渲染成"已发现"。

## 11. 节点资料 profile（v6 加法，2026-09-03）

本机节点的展示层资料（name/description/avatar）。定位：纯 GUI 展示属性，仅存本机
（app 数据目录 node-profile.json，原子写），不进底座、不随发现协议广播；对端资料
互通已于 2026-09-09 落地为按需拉取协议 /im/profile/1（wire-protocol.md §8.5），
GUI 经 §12.1 chat_peer_profile 查询，对端自报内容不做真实性验证。与 GuiConfig
完全独立：不进 node_start，保存后无需重启节点即生效。

```ts
interface NodeProfile {
  name: string;          // trim 后 ≤64 字符；空串 = 未命名（界面回退 PeerId 缩略/占位文案）
  description: string;   // ≤280 字符；可空
  avatar: string | null; // data URL：data:image/png|jpeg|webp;base64,…，总长 ≤200_000；null = 未设置
}
```

- profile_save 校验失败一律 Err（可读中文）：name/description 超长、avatar 超
  200_000 字符、MIME 不在 png/jpeg/webp 白名单、base64 载荷含非法字符。
- 校验通过原样落盘（后端不 trim，表单侧负责）；持久化层损坏/缺文件回退默认值并留
  warn 日志，禁止静默吞。
- 消费点：设置页资料卡（编辑入口）、侧边栏身份徽标（头像 + 名称展示）。

## 12. IM 聊天（v7 加法，2026-09-04，契约来源 docs/design/im-chat-design.md §3/§5）

好友间 1:1 私聊：好友簿管理、文本/emoji、图片/音频/视频/文件附件、消息历史分页、
发送状态可见、离线队列（outbox）、回复引用（replyTo 可选字段，IM-T46A 契约加法：
旧端忽略未知字段照常收信，不校验被引用消息存在性——离线引用允许）、好友分组
（group 可选字段，IM-T43 契约加法：单分组语义，None/空串 = 未分组，组名 trim 后
1..=32 字符；好友簿仅本地 friends.json，分组不进 ChatEnvelope，wire 协议不变；
CLI friends --group 同卡对齐。2026-09-08 GUI 去分组：通讯录好友平铺展示，
group 字段仅存不显，移动分组入口下线）。
实时通话/群聊/已读回执不在本轮。底座只读，全部落 crates/p2p-chat + src-tauri 消费面。

### 12.1 命令表（追加，全部 camelCase；参数无效一律 Err 可读中文）

| 命令 | 参数 | 返回 | 语义 |
|---|---|---|---|
| chat_friends_list | - | ChatFriendJson[] | 读好友簿（无文件返回空数组） |
| chat_friend_add | peerId: string, nickname: string, addrs: string[] | ChatFriendJson | 校验（peerId base58 且 ≠ 本机、nickname trim ≤64、addr 语法逐条校验）后原子写好友簿；addr 同时登记地址簿可拨 |
| chat_friend_remove | peerId: string | boolean | 从好友簿移除；never 在簿 → false（幂等），不删消息历史 |
| chat_friend_update | peerId: string, patch: { group?: string \| null; nickname?: string \| null; note?: string \| null } | ChatFriendJson | 资料补丁（IM-T43 加法）：group/nickname/note 至少一项，addrs 不可经此修改；group 空串 = 移出分组（归一化 null，不落盘空串）；组名 trim 后 ≤32 字符；空补丁或 peer 不在簿 → Err |
| chat_peer_profile | peerId: string | PeerProfileJson \| null | 对端节点资料按需拉取（2026-09-09 加法）：经 /im/profile/1（wire-protocol.md §8.5）查询对端自报资料 {name, description, avatar}；不可达类失败（离线/开流失败/应答超时）返回 null（展示层降级），参数非法与协议违规 → Err。内容为对端自报，展示层自行取舍 |
| chat_history | peer: string, beforeId?: string | null, limit?: number | ChatMessageJson[] | 按 time desc 分页，limit 默认 50 上限 100；beforeId 游标=严格更早 |
| chat_send | peer: string, kind: ChatKind, text?: string, media?: ChatMediaInput, replyTo?: string \| null | ChatSendReport | 校验→生成信封→落 outbox→尝试发送；文本 trim 后 1..=2000 字符；媒体原始字节 ≤64MiB；replyTo 提供时须非空字符串（不校验被引用消息存在性，离线引用允许） |
| chat_media_file | peer: string, messageId: string | { path: string; mime: string; name: string } | 返回附件落盘绝对路径（仅本端展示用）；消息非 media 或不存在 → Err |
| chat_media_export | sourceUrl: string, destPath: string, onProgress: Channel<{ receivedBytes: number; totalBytes: number }> | { destPath: string; totalBytes: number } | 媒体导出（2026-09-07 加法，详见 §12.5）：sourceUrl 为 chat_media_file/group_media_file 返回的 asset URL，destPath 为保存对话框产物；分块拷贝并经 Channel 回报进度 |
| chat_friend_invite
| chat_invites_list | - | FriendInviteJson[] | 邀请列表（out 待对方同意 / in 待本机处理） |
| chat_invite_accept | peerId: string, nickname: string | ChatFriendJson | 同意来邀：本侧立即建好友并回投 ACCEPT；nickname 空串 = 沿用邀请内对端自称；无来邀 → Err |
| chat_invite_reject | peerId: string | void | 拒绝来邀并通知对方（尽力而为）；无来邀 → Err |
| chat_invite_cancel | peerId: string | boolean | 撤回本机待同意邀请；无邀请幂等 false | | peerId: string, nickname: string, addrs: string[] | InviteReportJson | 发邀请：校验同 add；登记 out 邀请并尽力投递（delivered=送达/挂起）；重复邀请幂等刷新；已是好友 → Err |

### 12.2 事件（追加到 NodeEventJson 判别联合）

```ts
| { type: "chat_message"; peer: string; message: ChatMessageJson }   // 入站新消息（已落盘）
| { type: "chat_status"; peer: string; messageId: string; status: "pending"|"sent"|"delivered"|"failed" }
| { type: "chat_invite"; peer: string; state: "incoming"|"accepted"|"rejected" }
```

### 12.3 数据类型

```ts
type ChatKind = "text" | "image" | "audio" | "video" | "file";

interface ChatFriendJson {
  peerId: string;        // base58
  nickname: string;      // trim 后 ≤64；空串回退 PeerId 缩略
  addrs: string[];       // ip/u端口 = QUIC，ip/t端口 = TCP（对齐 §6 语法）
  note?: string | null;
  group?: string | null; // 分组名（IM-T43 加法）；null/缺省 = 未分组；单分组语义，UI 未分组虚拟组置底
}

interface ChatMediaInput {
  name: string;          // 原始文件名（展示用，落盘时 sanitize）
  mime: string;          // 小写；按 kind 白名单校验（见设计 §5），不匹配 Err
  dataBase64: string;    // 原始字节 base64（解码后 ≤64MiB，超限 Err）
}

interface ChatMediaJson {
  name: string;
  mime: string;
  size: number;          // 原始字节数
  path?: string | null;  // 本端落盘绝对路径（仅返回给本端消费）
}

interface ChatMessageJson {
  id: string;            // UUID（发端生成）
  peer: string;
  sender: "me" | "them";
  kind: ChatKind;
  tsMs: number;
  text?: string | null;
  media?: ChatMediaJson | null;
  status: "pending" | "sent" | "delivered" | "failed";  // 本地状态字段，不跨网
  replyTo?: string | null;   // 被引用消息的本端消息 id；null/缺省=无引用（IM-T46A 加法，不校验存在性）
}

interface ChatSendReport {
  message: ChatMessageJson;   // status=delivered=已实时送达；否则 pending（outbox 等待）
  delivered: boolean;
  flushedOutbox?: number;     // 本轮命令顺手补投的历史积压条目数；0/缺省=无补投（CLI 演练加法）
}
```

- 持久化位置：`<dataDir>/chat/`（friends.json / outbox/<peer>.jsonl /
  messages/<peer>.jsonl / media/<peer>/<msgId>_<sanitizedName>），介质权限与原子写对齐 §11 纪律。
- 媒体预览：`chat_media_file` 返回 path 后，前端经 Tauri asset protocol（assetProtocol
  scope 须含 chat/media 目录，src-tauri 侧接线）内联展示 image/audio/video；file 展示
  名称/大小。下载入口见 §12.5（保存对话框导出，替代早期锚点直开）。系统级"打开默认应用"不在本轮契约内。

### 12.5 媒体导出（2026-09-07 加法）

- 命令：`chat_media_export(sourceUrl, destPath, onProgress)`（§12.1 表同）。
- 来源解析：src-tauri 内部把 asset URL 还原为本端落盘绝对路径（`util::to_asset_url`
  逆变换），并 canonicalize 校验必须位于应用数据目录内——前端永不直传路径，防任意文件外拷。
- 目标路径：前端经 tauri-plugin-dialog `save()` 系统保存对话框取得（默认文件名 = 附件
  原始名），取消（null）则不发命令。
- 进度：64KiB 分块拷贝，每 ≥256KiB 经 `Channel` 回报一次 + 收尾必发终态；前端全局
  download-store 记任务（key = sourceUrl），气泡内联进度条展示，切会话/页面不丢。
- 失败语义：来源越界/不可读/写目标失败 → Err 可读中文；进度回报失败（前端已离开）
  仅告警不中断导出。
- CLI 对等：cli-parity.tsv 登记为 exempt（对话框与 Channel 为桌面壳专属，无 CLI 等价面）。
- 验收对齐点：A 侧 serde 字段名与上表逐字一致（camelCase，Option 序列化 null）；
  B 侧 TS 类型与上表逐字一致；mock 与真实实现同签名。


### 12.4 好友邀请（v9 加法，2026-09-05，邀请制加好友）

加好友唯一用户路径 = 发邀请 → 对方同意 → 双向互为好友。直加接口移除（crate
保留 friend_add_direct 仅供测试引导，不进命令面）。wire 协议 /im/invite/1
登记见 wire-protocol.md §8.2；邀请簿本地 invites.json。

### 12.4.1 命令表（追加）

| 命令 | 参数 | 返回 | 语义 |
|---|---|---|---|
| chat_friend_invite | peerId: string, nickname: string, addrs: string[] | InviteReportJson | 发邀请：校验同 add；登记 out 邀请并尽力投递（delivered=送达/挂起）；重复邀请幂等刷新；已是好友 → Err |
| chat_invites_list | - | FriendInviteJson[] | 邀请列表（out 待对方同意 / in 待本机处理） |
| chat_invite_accept | peerId: string, nickname: string | ChatFriendJson | 同意来邀：本侧立即建好友并回投 ACCEPT；nickname 空串 = 沿用邀请内对端自称；无来邀 → Err |
| chat_invite_reject | peerId: string | void | 拒绝来邀并通知对方（尽力而为）；无来邀 → Err |
| chat_invite_cancel | peerId: string | boolean | 撤回本机待同意邀请；无邀请幂等 false |

### 12.5 数据类型（12.4 追加）

```ts
type InviteDirectionJson = "out" | "in";
type InviteStateJson = "incoming" | "accepted" | "rejected";

interface FriendInviteJson {
  peerId: string; nickname: string; addrs: string[]; note?: string | null;
  direction: InviteDirectionJson; tsMs: number; delivered: boolean;
}

interface InviteReportJson { invite: FriendInviteJson; delivered: boolean }
```

## 13. 应用内下载安装更新（v8 加法，2026-09-04，G-U3）

§9 的检查提醒保持不变；本节新增「程序内下载 + 进度条 + 下载成功自动安装 + 重启」闭环。
实现采用官方 tauri-plugin-updater（minisign 签名校验）与 tauri-plugin-process（relaunch），
不自研安装器；Rust 侧只注册插件，无新增命令。

- updater 端点（编译期常量，tauri.conf.json plugins.updater）：
  `https://github.com/imeepos/p2p/releases/latest/download/latest.json`。
  清单由 ci(gui-client.yml) release job 发布时生成（apps/gui/scripts/release/make-latest-json.mjs）：
  四平台签名增量包缺一或签名不成对即发布失败；macOS 双架构增量包同名，就地加架构后缀
  改名规避 release 资产重名（签名只覆盖文件内容，与文件名无关）。
- 签名：minisign 密钥对。公钥入库（plugins.updater.pubkey）；私钥只存在于 CI secret
  （TAURI_SIGNING_PRIVATE_KEY，无密码）与本机 .env（TAURI_SIGNING_PRIVATE_KEY_PATH），
  严禁入库。bundle.createUpdaterArtifacts=true 后无私钥 tauri build 直接失败，
  机制上杜绝未签名增量包进入 release。
- 前端命令面（ipc.ts 第三命令面 updateDl，与 ipc/diag 并列；mock 同签名，视图禁直连插件包）：

```ts
interface RemoteUpdate {
  version: string;       // 远端新版本号
  notes: string | null;  // 更新清单 notes（当前清单不含，保留扩展位）
}
interface UpdateDownloadProgress {
  downloadedBytes: number;
  totalBytes: number | null; // Started 事件可能缺 contentLength，此时进度不定态
}
interface UpdateDownloadBackend {
  checkRemoteUpdate(): Promise<RemoteUpdate | null>; // null = 已是最新
  // 下载并自动安装；onProgress 按块回调；完成后 resolve
  downloadAndInstallUpdate(onProgress: (p: UpdateDownloadProgress) => void): Promise<void>;
  relaunchApp(): Promise<void>;
}
```

- 状态机（update-store）：idle → downloading（进度按块推进，百分比/字节双展示）→
  installed →（用户点「立即重启」）relaunch；失败落 failed + 可读中文错误可重试。
- 发起条件：仅 §9 status=available 时可发起；in-flight 防抖（downloading 期间重复发起
  忽略）；downloadAndInstall 前先经 updater 端点重新取更新句柄，不跨轮询周期持旧句柄。
- 重启时机归用户：安装完成不自动重启；Windows NSIS 静默安装器可能自行退出并重启应用，
  属平台行为，前端不做补偿。
- 平台覆盖：macOS（.app 替换 + relaunch）、Windows（NSIS zip，MSI 不走 updater）、
  Linux（AppImage 替换；deb 包用户继续走 §9 浏览器手动下载）。
- 逃生通道：§9 的 update_open_release_page（浏览器打开发布页）在所有相位保留。
- 失败语义：下载/签名校验/安装失败一律可读中文并留 console 与日志，禁止静默吞；
  未打包二进制（pnpm tauri dev）不启用真实安装；浏览器 dev 走 mock（VITE_MOCK_IPC=1），
  mock 与真实实现同签名。

## 14. IM 群聊（v9 加法，2026-09-04，契约来源 docs/design/im-group-design.md §5/§7）

群 = 好友边沿上的多播：owner 为名单唯一权威（rev 单调递增，成员被动接收 roster 并落盘），
消息按成员沿既有 1:1 链路 fan-out，离线走 goutbox 双队列（PeerConnected flush、批量上限
32、二次死信出队 + 告警）；历史分页/附件/回复引用复用 §12 语义。硬边界：群 ≤32 人、
单条 ≤64MiB、群名 trim 后 1..=64 字符、成员必须是好友簿在册节点；groupId = UUID
（owner 生成）。命令参数无效一律 Err 可读中文。

### 14.1 命令表（追加，全部 camelCase）

| 命令 | 参数 | 返回 | 语义 |
|---|---|---|---|
| group_create | name: string, memberIds: string[] | GroupJson | 校验（成员 ⊆ 好友簿、≤32、不含本机、群名 trim 1..=64）后本地建群并推 roster |
| group_list | - | GroupJson[] | 全量含 left/kicked/disbanded（GUI 按 state 过滤/置底） |
| group_invite | groupId: string, memberIds: string[] | GroupJson | owner-only；rev+1 推全体（含新成员） |
| group_kick | groupId: string, memberId: string | GroupJson | owner-only；rev+1 推余员 + G_KICK |
| group_leave | groupId: string | GroupJson | 本端 state=left；G_LEAVE 通知 owner |
| group_rename | groupId: string, name: string | GroupJson | owner-only；rev+1 推 roster |
| group_disband | groupId: string | GroupJson | owner-only；rev+1，对全体其他成员发 G_KICK(reason=disbanded)；本端 state=disbanded；非 active 重复解散显式 Err |
| group_send | groupId, kind: ChatKind, text?, media?: ChatMediaInput, replyTo? | GroupSendReport | 校验→fan-out；见设计 §6 |
| group_history | groupId: string, beforeId?: string, limit?: number | GroupMessageJson[] | 同 1:1 分页语义 |
| group_media_file | groupId: string, messageId: string | { path, mime, name } | 同 1:1，目录为 media/<groupId>/ |

### 14.2 事件（追加到 NodeEventJson 判别联合）

```ts
| { type: "chat_group_message"; groupId: string; message: GroupMessageJson }
| { type: "chat_group_status"; groupId: string; messageId: string; acks: string[]; status: "pending"|"delivered"|"failed" }
| { type: "chat_group_state"; group: GroupJson }   // roster 变更/踢出/解散/退群回执
```

### 14.3 数据类型

```ts
interface GroupJson {
  groupId: string;        // UUID
  name: string;           // trim 后 1..=64 字符
  owner: string;          // PeerId
  members: string[];      // PeerId[]，含 owner，≤32
  rev: number;
  state: "active" | "left" | "kicked" | "disbanded";
  tsMs: number;
}

interface GroupMessageJson {
  id: string;             // UUID（发端生成）
  groupId: string;
  senderId: string;       // 作者 PeerId；本端消息判定 senderId === 本机 PeerId
                          //（GUI 经既有节点信息命令取本机 PeerId，渲染路径复用 1:1 气泡）
  kind: ChatKind;
  tsMs: number;
  text?: string | null;
  media?: ChatMediaJson | null;   // 复用 §12.3 ChatMediaJson
  status: "pending" | "sent" | "delivered" | "failed";  // sent 不出现（设计 §4 状态机）
  acks: string[];         // 已确认成员 PeerId（仅本端发出的消息非空）
  replyTo?: string | null;
}

interface GroupSendReport {
  message: GroupMessageJson;
  acked: number;          // 本轮已确认成员数
  recipients: number;     // 目标成员数（n-1）
  delivered: boolean;     // acked === recipients
}
```

- ChatKind / ChatMediaInput / ChatMediaJson 复用 §12.3；群媒体预览同 §12 asset
  protocol 纪律（scope $APPDATA/chat/media/**/* 通配群子目录 media/<groupId>/）。
- 持久化：dataDir/chat/ 增量且 1:1 文件零迁移——groups.json（全量群，四态
  active/left/kicked/disbanded；退群/被踢/解散不删数据）、goutbox/<peer>.jsonl
  （群 per-member 离线队列）、groups/<groupId>.jsonl（群历史）、media/<groupId>/。
- 送达展示：GroupSendReport 的 acked/recipients/delivered 与 GroupMessageJson.acks
  推导「已送达 |acks|/n」；sent 状态不用于群消息（枚举保留不占用）。
- 实现登记（G6 升格，2026-09-05）：group_disband 已列入 §14.1 命令表（设计
  §5 语义：owner 校验、rev+1、对全体其他成员发 G_KICK(reason=disbanded)、本端
  state=disbanded；非 active 重复解散显式 Err）。G2 九命令面缺口闭环：ipc/mock/
  store 与群管理面板均接真命令，逐个 groupKick 变通移除。
- 验收对齐点：A 侧 serde 字段名与上表及事件逐字一致（tests/group_contract.rs
  矩阵断言，Option 序列化 null、acks 缺省容忍旧记录）；B 侧 TS 类型与上表逐字一致
  （ipc-types.ts / ipc.ts 九方法）；mock 与真实实现同签名（mock-group-roster）；
  命令层 group_create/group_send 双回环真节点冒烟见 tests/group_command_smoke.rs。

## 15. acp 泵进程内装配（v15 重写，2026-09-08，INLINE-ACP-PUMP 波；取代 v10 的 sidecar 托管语义）

GUI 壳在进程内装配 acp-pump（crates/acp-pump，根 workspace 成员），无外部进程，
无定位/监督/重启语义。语义与验收对齐点：

- 装配：GUI setup 即 `Pump::start`（crates/acp-pump runtime 句柄面），数据目录
  仍为 app_data_dir/acp-console-data；装配失败转 disconnected 并留 lastError，
  不阻断 GUI 主功能（R3 降级先例）。
- 泵任务异常收口：panic/运行期错误由 acp-pump catch_unwind 收口为
  `PumpExit::failed`，GUI 一律转 disconnected + lastError 显式失败态，禁止静默；
  GUI 退出（RunEvent::Exit）幂等停泵（handle.stop → node.shutdown）。
- CLI 对等：`p2pctl acp console` 前台跑同一装配函数（参数面 bootstrap/data-dir/
  ws-port/status-port/share-link 等，就绪信息经 stdout JSON 行发布）；状态快照
  查询对等 `p2pctl acp status`（经 pump status HTTP /status 同词汇）。

命令（§1 表；命令名沿 v10 保留，语义改为 in-process pump 状态）：

| 命令 | 参数 | 返回 | 语义 |
|---|---|---|---|
| acp_console_status | - | AcpConsoleStatus | pump 状态快照（connected 携带连接面） |

事件通道 acp-console（独立于 node-event，通道名沿 v10 保留）：payload 即
AcpConsoleStatus，phase 变更即发射，可携带可选 tsMs（§2 同款约定）。

```ts
interface AcpConsoleStatus {
  phase: "connecting" | "connected" | "disconnected";
  wsUrl?: string;      // connected 后：ws://127.0.0.1:<port>
  token?: string;      // connected 后：console WS 鉴权 token
  statusUrl?: string;  // connected 后：pump status HTTP 地址
  lastError?: string;  // 最近一次失败原因（可读）
}
```

- 验收对齐点：A 侧真实 Pump::start 回环（connected 连接面/数据目录/停机迁移/
  退出原因映射）+ 命令 serde roundtrip；B 侧 mock 改 pump 内部状态三态，与真实
  实现同签名；cli-parity 的 acp_console_status 映射 p2pctl acp status（v15）。
- wire 契约真值源：WS 哑泵帧面/status HTTP 端点/share-link 解析 =
  crates/acp-pump（src/ws.rs、src/status.rs、src/share/link.rs）；原
  apps/acp-console 独立子项目已删除（T5），`?proto=a2a` 事件通道语义不变（§17.1）。



## 16. llm-share GUI 面（v11 加法，2026-09-06，LSG 波；语义真值源=docs/ops/p2pctl-ai-guide.md 九条目+PR 轨冻结稿 v1）

### 16.1 命令面（src-tauri 封装 crates/llm-share-*，chat.rs 先例；PR4 borrow 注释行随本面 live 行迁移同提交串）

| 命令 | 参数 | 返回 | 语义 |
|---|---|---|---|
| llm_share_offer_publish | offer | LlmOfferView | 发布出借声明（签名信封落 offer.json）；必填集：models ≥1、spare 覆盖全部 model 且 N>0、period-ends 日期——表单契约显性化，IPC 层校验 |
| llm_share_offer_show | - | LlmOfferView | status 五态 live/expired/not_yet_valid/peer_mismatch/bad_signature |
| llm_share_allow_list | - | { entries: LlmAllowEntry[] } | 白名单清单 |
| llm_share_allow | peerId, models?: string[], note? | LlmAllowlistView | models 缺省=不限模型（原话）；deny 不存在条目=显式报错非错误态 |
| llm_share_deny | peerId | LlmAllowlistView | 移除白名单 |
| llm_share_borrow | req{model, messages(纯文本或 OpenAI 数组 JSON，非数组由 IPC 层包装为单条用户消息), maxTokens(必填), targetPeer(必填), reqId?} | LlmBorrowReport | targetPeer 无缺省路径，缺出借方=IPC 层显式报错；reqId 客户端生成 UUID 重试复用，缺省 IPC 层生成 |
| llm_share_ledger_list | filter{lender?, borrower?, period?} | LlmLedgerEntry[] | 账本流水 |
| llm_share_ledger_balance | - | LlmBalanceGroup[]{lender, period, lentOut, borrowed, netAmount, entries, direction(lent/borrowed/flat)} | 净差按 lender+period 切分，正负号=借贷方向；lentOut/borrowed/entries 供明细行插值 |
| llm_share_receipt_verify | reqId, lenderPubkey? | LlmReceiptVerifyResult | 缺省本机身份仅出借方自验；借方场景须传出借方公钥或从账本条目取 |

LlmBorrowReport{status: done|stream_broken|rejected, receipt{reqId, appended, estimated, disputeWindowSecs}, sseCount, usage?, code?, message?}

### 16.2 语义约束（违约即验收红）

1. 拒绝码四值 not_allowlisted/model_not_served/freeze_insufficient/concurrency_exceeded 原样透出不本地化改写；rejected 是业务结果非命令 Err。
2. stream_broken：estimated=true、disputeWindowSecs=72h、退出 0——GUI 渲染「估算账单·72h 争议窗」中性态，禁渲染失败。
3. req_id 幂等：重试复用同 reqId，appended=false 不双记。
4. 数据文件 GUI 只读展示，禁直写 ledger.json/receipt-*/offer.json/allowlist.json。
5. offer status：expired/not_yet_valid=常态中性；peer_mismatch/bad_signature=醒目警示。
6. borrow=真实成本动作：UI 二次确认+maxTokens 显式上限必填；sse 原文只出 sseCount，正文截断展示。
7. 默认拒绝心智进 UI 文案原话：「allowlist 无条目即不可用」。

### 16.3 落点

/llm-share 独立路由页四面板（offer 发布/allowlist 管理/borrow 快捷/双边账本视图）；rail 四入口不动（/docs 先例：命令面板+设置页入口可达）；设置页增 llm-share 入口卡。

v12 加法（2026-09-07）：上游配置第五面板——本地自用 provider 配置列表（名称/baseUrl/apiKey/模型，仅存 GUI localStorage，键 p2p-gui-llm-providers；不触 §16.2-4 账本文件）。分享动作 = offerPublish（按配置模型）+ allow（好友按同批模型放行）；apiKey 不进任何 IPC 请求与展示（列表只出掩码），即「分享可用性而非密钥」。

v12 取代注记（2026-09-08，llm-share-link 波）：§16.6 v13 将其取代——provider 配置存储从
localStorage 上移为本机持久化（providers.json + 0600 密钥文件），分享动作改为 dsh-llm-share://
临时链接（share_redeem 兑换进 allowlist）。旧 localStorage 数据一次性幂等迁移；provider-share-form
手动分享入口移除。v12 的命令面/语义以 §16.6 为准，本段保留为历史。

### 16.4 cli-parity 迁移

GUI 命令 live 行与 PR4 borrow 注释行升级同一提交串落地，中间态守卫不红；ai-docs-sync 联动由 GUI 轨验收把关。

### 16.5 §3 加法

GuiConfig 增 lanOnly?: boolean（serde default，缺省 false；PR4 已落 serde 双向兼容）；设置页 lanOnly 开关纳入实现卡。

### 16.6 v13 加法（2026-09-08，llm-share-link 波；取代 v12，设计=docs/design/llm-share-link-design.md v2）

双协议 provider + 聊天分享链接全链。语义红线：

1. provider 配置本机持久化：providers.json（只存 id/name/baseUrl/protocol/models/createdAt + apiKeyRef，
   不存明文 key）+ 0600 密钥文件 keys/<id>.key；apiKey 明文仅入参、禁进日志/wire/台账/链接/argv；
   http:// baseUrl 显式告警；provider_remove 级联删 key 文件；localStorage 旧数据一次性幂等迁移。
2. 分享=临时链接：dsh-llm-share://v1?peer&addr&token&exp&sid&models；token=128-bit CSPRNG hex，
   台账只存 sha256，原文只在 share_create 响应出现一次；models 必填非空且 ⊆ offer.models；
   maxActivations 固定 1；exp 默认 24h 上限 7d。
3. 兑换=认证 PeerId 进 allowlist（source=share:<shareId>，模型集限定，expires_at=链接 exp）；
   兑换激活锁内 read-check-write 防 TOCTOU；手工条目优先、redeem 不覆盖、同 peer 模型取交集；
   revoke 按 source 级联删 allowlist 条目；allowlist 条目到期经 admit 惰性清理。
4. 出借方常驻 serve：node_start 装配（offer+providers 启动快照）、node_stop 卸载；
   serve_status.assembled:false 是常态非故障；入站身份取握手认证 PeerId（handle_inbound），
   帧内自报不信；幂等索引持久化（ledger.json append Receipt，启动重建 settled 防 req_id 双记账）。
5. 新增命令面 8 条（表）：

| 命令 | 参数 | 返回 | 语义 |
|---|---|---|---|
| llm_share_provider_list | - | { providers: LlmProviderView[] } | apiKey 只出掩码；损坏存档=显式报错回空不静默 |
| llm_share_provider_save | config{id?, name, baseUrl, protocol:openai\|claude, apiKey, models[]} | LlmProviderView | id 缺省生成；apiKey 明文仅入参落 0600 密钥文件；name/baseUrl/models 必填显性报错 |
| llm_share_provider_remove | providerId | { removed: true } | 不存在=显式报错非错误态；级联删 key 文件 |
| llm_share_share_create | req{providerId, models?, expiresAt?, maxActivations?, note?} | { link, shareId, expiresAt, models } | models 缺省=provider 全模型且须 ⊆ offer.models；maxActivations 固定 1；token 原文只在这条响应出现一次 |
| llm_share_share_list | - | { shares: LlmShareEntry[] } | 永不含 token/明文 key；status 推导 active/expired/revoked/exhausted |
| llm_share_share_revoke | shareId | { revoked: true } | 按 source=share:<id> 级联删 allowlist 条目；不存在/已撤销=显式报错 |
| llm_share_share_redeem | link | LlmShareRedeemResult{offer:{peer,models,spare,periodEnds}, shareId, owner} | 业务拒绝码 share-revoked/expired/exhausted/bound-other/invalid 原样透出非 Err；scheme/peer/token 缺失=参数错误显式 Err |
| llm_share_serve_status | - | LlmServeStatus{ assembled, providerId?, models[], lastError? } | assembled:false 是常态非故障；lastError 供面板显式告警 |

6. CLI 对等：7 条 mapped（provider list/save/remove、share create/list/revoke/redeem，逻辑进
   crates/p2p-cli 共享事实源，apps/cli clap 映射 + ai-guide 条目同卡）；serve_status exempt
   （serve 生命周期跟随 GUI 常驻节点，无 CLI 常驻进程面，登记 cli-parity.tsv 带 reason，
   acp_console_status 先例）。

## 17. A2A 智能体面（v14 加法，2026-09-08，A2A3 波；设计=docs/design/a2a-over-p2p-design.md §5.1/§7.4/§8）

GUI /agents 页（发现/我的双视图）的数据面契约。IPC 命令表零加法：本机 agent 凭
§15 AcpLocalDescriptor（adminUrl/token/peer），console 连接面凭 §15 AcpConsoleStatus
（wsUrl/token）；node-event 判别联合（NodeEventJson）不动（拍板 Q6）。

### 17.1 卡片/邀请事件通道（acp-pump WS 事件通道，proto=a2a）

- 通道 = `ws://127.0.0.1:<ws_port>/?token=&peer=<宿主PeerId>&proto=a2a`（crates/acp-pump
  src/ws.rs 为 wire 契约权威，原 apps/acp-console 独立子项目已随 INLINE-ACP-PUMP 撤销，
  泵面内联入 crates/acp-pump）。Pump 按 `proto` 拨 `/a2a/1`（缺省 `acp` 拨 `/dsh-acp/1`，
  未知值 401 显式拒绝），握手后纯字节泵，帧面真值源 = `crates/a2a` CardFrame（§5.1 表）。
- GUI 通道纪律：连上先 `list` 拉全量再 `subscribe`；`cards`/`push` 按
  `hostPeer/agentId` 键入簿，同键 version 升序才覆盖；`push.removed` 即除名；
  `error` 帧原样上浮 UI（不静默）。断线按既有 WS 重连节奏重拨并重放 list+subscribe。
- 邀请事件（A2A5）为同一通道的宿主→GUI 通知帧预留扩展位，不再加新协议 ID。
- 任务相（design §5.2，Q10「1 task = 1 流」）：泵连接字节透传即流本体，故 GUI
  每个任务独开一条本通道连接（`peer=<目标宿主>`），首帧 `tasks/create` 建附，
  同连接续发 `tasks/send|cancel`；`tasks/status|message` 通知按 taskId 入簿，
  create 应答与入簿窗口内的通知缓冲重放（禁止静默丢弃）。GUI 侧实现 =
  `apps/gui/src/a2a/task-socket.ts` + `a2a-store.ts`（2026-09-09 修复接线缺失：
  a2a 会话发送此前未连任务通道，且 composer 把非报告返回值当 ChatSendReport 判
  delivered 炸 TypeError 误报「发送失败」）。

### 17.2 本机 agent 管理面（「我的」视图，Bearer 鉴权，宿主 share admin 管道）

| 方法/路径 | 请求体 | 应答 | 语义 |
|---|---|---|---|
| GET /a2a/agents | - | `{"agents":[AgentDef…]}` | 我发布的定义全集 |
| POST /a2a/agents | `{agentId?, name, description, skills?, visibility}` | AgentDef | 创建即签名发布并广播 push；skills ≤10 条（id [a-z0-9-]）；name/description 非空 |
| PUT /a2a/agents/{id} | `{visibility?}` 或 `{enabled?}` | AgentDef | 变更即重签广播；名称/描述/技能 v1 数据面不可改（GUI 编辑态只开放可见性） |
| DELETE /a2a/agents/{id} | - | `{"removed":agentId}` | 下架即广播 removed（键 hostPeer/agentId） |
| POST /a2a/agents/{id}/invite | `{inviteePeer, expirySecs?}` | `{"invite":Signed<InvitePayload>}` | 生成签名邀请帧（A2A5），落盘邀请簿；expirySecs 默认 24h，上限 24h；inviteePeer 必须为合法 base58 PeerId |

- 错误面：400 invalid-json；404 unknown agent；422 校验拒绝（error 原文上浮 UI，禁静默）。
- AgentDef（camelCase）：agentId/name/description/skills[{id,name,…}]/visibility(public|private|local)。

### 17.3 GUI 语义约束（违约即验收红）

1. 在线点为纯前端启发（卡过期时刻 = issued_at+ttl_secs）：剩余 TTL >50% 绿、≤50% 黄、
   过期灰；真实心跳面接入前不得展示「在线/离线」文案，仅色点。
2. 可见性徽章：public=绿、private=灰、local=灰（设计 §8.2）。
3. 「聊天」（?a2a= 会话）已交付（A2A4）：点击导航到 /chat?a2a=<agentKey>，
   右侧渲染 A2aConversation 组件（task 语义，与 ACP transcript 栈分家）。
   「分享」（生成签名邀请）已交付（A2A5 + 收口波）：POST /a2a/agents/{id}/invite
   生成签名邀请帧，ShareInviteDialog 展示帧 JSON 供复制。
4. skills 输入 chip ≤10 条、trim+去重（拍板 Q9）；校验失败原位上浮。

## 18. 好友角色管理（v16 加法，2026-09-09，authz S3；设计=docs/design/authz-role-design.md §5/§7/§10 + authz-a3-plan §1 S3）

通讯录好友页的角色管理面：为好友绑定/解绑角色（内建四 + 自定义，含过期时间）、
查看当前绑定与判定语义、配置加好友默认角色（default_role）。逻辑层复用
`p2p-cli` authz 管理面（role_list/bind/unbind/check 及报告形状，与 p2pctl authz
同词汇同事实源）；判定与存储委托 crates/p2p-authz（四步瀑布/原子写/损坏显式
报错）。数据根 = GUI app 数据目录下 `authz/`（CLI `--data-dir/authz/` 等价物，
同 §16 llm-share 口径；非 GuiConfig.dataDir 节点数据目录），与 CLI 共用
roles.json/bindings.json/audit.jsonl。命令参数无效一律 Err 可读中文。

### 18.1 命令表（追加，全部 camelCase；可选参数统一传 null）

| 命令 | 参数 | 返回 | 语义 |
|---|---|---|---|
| authz_role_list | - | { roles: AuthzRoleView[] } | 内建四角色 + 自定义角色全量（builtin 字段区分）；自定义角色经 CLI `authz role create` 创建（GUI 本轮不做创建入口） |
| authz_bindings_list | - | { bindings: AuthzBindingJson[] } | 当前绑定全集（peer 级单值）；好友页按 peerId join 渲染角色徽章 |
| authz_bind | peerId: string, roleId: string, expiresAt?: number \| null, note?: string \| null | AuthzBindReport | upsert 绑定（单值语义，改绑即覆盖）；expiresAt 为 Unix 秒（缺省 = 不过期，到期后判定 Deny(Expired)）；roleId 必须已登记；成功落 authz.bound 审计事件 |
| authz_unbind | peerId: string | AuthzUnbindReport | 解绑（移除条目）；无绑定 → Err；成功落 authz.unbound 审计事件 |
| authz_check | peerId: string, permission: string | AuthzCheckReport | dry-run 判定，不写任何状态；无角色绑定 = deny(NotBound)（默认拒绝可观测）；读失败 = Err 显式报错不静默放行（设计 §11 红线 2） |
| authz_default_role_get | - | { roleId: string } | 读加好友自动绑角色（配置字段 authzDefaultRole）；空串 = 已禁用自动绑 |
| authz_default_role_save | roleId: string | { roleId: string } | 写 authzDefaultRole：空串 = 禁用；非空必须为已登记角色 id（内建或自定义）；原子女写持久化配置，不改运行中节点 |

### 18.2 数据类型

```ts
interface AuthzRoleView {
  roleId: string;        // 内建: friend/guest/operator/ally；自定义: [a-z0-9-]{1,32}
  name: string;          // 展示名
  permissions: string[]; // §4 闭集 key（chat.send/a2a.invoke/llm.borrow 等）
  builtin: boolean;      // 内建四 = true（不可改不可删）
  note: string;
}

interface AuthzBindingJson {
  peerId: string;
  roleId: string;
  grantedAt: number;  // Unix 秒
  note: string;
  expiresAt?: number; // Unix 秒；无过期时字段不出现（对齐 p2p-cli 报告 skip-none 先例）
}

interface AuthzBindReport {
  peerId: string;
  roleId: string;
  created: boolean;   // false = 已有绑定，本次为 upsert 更新
  grantedAt: number;  // Unix 秒
  expiresAt?: number; // 同上，无过期不出现
  note: string;
}

interface AuthzUnbindReport { peerId: string; roleId: string }

interface AuthzCheckReport {
  peerId: string;
  permission: string;
  decision: "allow" | "deny";
  reason: "NotBound" | "Expired" | "BrokenRole" | "MissingPerm" | null; // deny 时非 null
}
```

### 18.3 §3 加法

GuiConfig 增 `authzDefaultRole?: string`（serde default 缺省 "friend"，可设其余
内建或自定义角色 id；空串 = 不自动绑）。CLI 侧镜像（apps/cli types.rs）已先行
落地同名字段；GUI 配置读写（§1 config_get/config_save）与本节 default_role
get/save 消费同一持久化文件，双向兼容旧配置（缺字段补默认，不覆盖用户已设值）。

### 18.4 语义约束（违约即验收红）

1. 审计事件（authz.bound/authz.unbound）由 p2p-cli 逻辑层经 p2p-authz audit
   sink 落 `<app 数据目录>/authz/audit.jsonl`（A3 P1b 同一份，禁双写）；审计
   不进 GUI 展示面（无事件通道、无 UI 呈现）。
2. 默认拒绝心智进 UI：无绑定好友在角色编辑对话框呈现「未绑定 = 全域拒绝」
   中性态；徽章仅展示角色名，不渲染权限矩阵判定结果。
3. 角色下拉数据源 = authz_role_list（内建四 + 自定义，builtin 置灰不可删改
   提示）；过期时间输入为可选 Unix 秒（GUI 提供常用时长快捷换算，允许直填）。
4. 错误语义：peerId 非法（base58 解码非 32 字节）/ roleId 未登记 / permission
   不在闭集（错误信息附可用 key 清单）/ 解绑无绑定 peer / default_role_save
   未登记角色 / 存储损坏或读失败 → 一律 Err 可读中文；无静默回退。
5. 验收对齐点：A 侧 serde 字段名与上表逐字一致（camelCase，BindReport/
   CheckReport 复用 p2p-cli 报告类型）；B 侧 TS 类型与上表逐字一致；mock 与
   真实实现同签名；CLI 对等面（p2pctl authz role list/bind/unbind/check）
   登记 cli-parity.tsv mapped，default_role 两条为 exempt（配置字段读写，
   CLI 直编辑 gui-config.json 无独立子命令，理由随表登记）。

## 19. 隧道本地反代面（v17 加法，2026-09-11，W-T1 契约；线格式真值源=docs/protocol/specs/tunnel.md，协议 `/p2p-base/tunnel/1`）

DSH 启动 URL 驱动的远程 Web 访问：GUI 收到 `http://127.0.0.1:<port>/?token=<tok>`
形状的启动 URL 后，经隧道协议连到被访节点，把其本机 `127.0.0.1:<port>` 服务映射到
本机回环随机端口，拼出 `open_url = http://127.0.0.1:<local_port>/?token=<同 token>`
供系统浏览器打开。协议语义（票据帧/应答帧/错误码闭集/半关闭/审计）以规范页为准，
本节只冻结 GUI 面。事件采用独立 Tauri 事件名 `tunnel_status`（`emit("tunnel_status",
TunnelStatusReport)`，前端 `listen<TunnelStatusReport>("tunnel_status", ...)`）：
不改 NodeEventJson 判别联合（§2），避免在节点通用事件通道里混入隧道高频状态。

### 19.1 命令表（追加；JSON 字段一律 camelCase）

| 命令 | 参数 | 返回 | 语义 |
|---|---|---|---|
| tunnel_open_dsh | url: string | TunnelOpenReport | 解析 DSH 启动 URL（host 必须为字面量 `127.0.0.1`，token/path 原样保留）→ 以本地生成的 16 hex uid 开 `/p2p-base/tunnel/1` 流（票据 target = URL 的 `127.0.0.1:<port>`）→ `bind(127.0.0.1:0)` 起本地反代 → 回 `{local_addr, open_url, token}`；host 非回环字面量/URL 解析失败/隧道被拒（error 帧）一律 Err 可读中文（含错误码闭集 code） |
| tunnel_status | - | TunnelStatusReport | 当前隧道会话快照；无活动会话时 `active=false`、`localAddr/target=null`、`sessions=[]` |
| tunnel_serve_start | target: string | TunnelServeStatus | 被访侧服务开关：把 `127.0.0.1:<port>` 加入目标白名单（累积，重复调用即多项）并开启受理（enabled=true）；target 非字面量 `127.0.0.1` / 端口非法 → Err 可读中文，先校验后动作，不得部分生效（白名单不变更、enabled 不翻转） |
| tunnel_serve_stop | - | TunnelServeStatus | 被访侧关闭受理（enabled=false），白名单保留不清空；此后不再接受新建流；既有会话不强制中断，按规范页 §3.2 语义自然收尾，且每条必须落终态（emit tunnel_status，outcome ≠ "open"） |

### 19.2 事件与数据类型

| 事件 | payload | 语义 |
|---|---|---|
| tunnel_status | TunnelStatusReport | 任一侧状态变更时后端主动 emit（访侧会话建立/关闭/出错；被访侧 enabled/allow/activeSessions 变化）；会话终态（outcome ≠ "open"）必发一次，UI 据此提示，错误不静默。复用同一事件名而非新增 `tunnel_serve_status`：一次订阅看全两侧，payload 经 `serve` 字段保持单一形状，判别不靠猜测 |

```ts
interface TunnelOpenReport {
  localAddr: string;  // "127.0.0.1:<local_port>"（bind(127.0.0.1:0) 随机端口）
  openUrl: string;    // http://127.0.0.1:<local_port>/?token=<同 token>
  token: string;      // 原样透传 DSH 启动 URL 的 token
}

interface TunnelStatusReport {
  active: boolean;           // 存在未关闭会话
  localAddr: string | null;  // 本地反代监听地址；无活动会话 = null
  target: string | null;     // 被访目标 127.0.0.1:<port>；无活动会话 = null
  sessions: TunnelSessionAudit[];
  serve: TunnelServeStatus;  // 被访侧服务面（v17 补件加法字段，emit 恒带）
}

interface TunnelServeStatus {
  enabled: boolean;        // 受理开关；默认 false，会话态不持久化，重启回落关闭
  allow: string[];         // 目标白名单（127.0.0.1:<port>，累积，持久化）
  activeSessions: number;  // 被访侧视角存续会话数
}

interface TunnelSessionAudit {
  sessionId: string;        // 票据 uid（16 hex，两侧日志同源）
  peerId: string;           // 被访节点 PeerId（base58）
  target: string;           // 127.0.0.1:<port>
  startedAt: number;        // Unix 秒
  endedAt: number | null;   // Unix 秒；未结束 = null
  bytesIn: number;          // 记录方视角：自隧道收到
  bytesOut: number;         // 记录方视角：向隧道发出
  outcome: "open" | "ok" | TunnelErrorCode;
}

type TunnelErrorCode =
  | "bad_ticket" | "target_not_allowed" | "busy"
  | "dial_failed" | "io" | "shutdown";   // 规范页 §4 六值闭集，禁增删
```

### 19.3 语义约束（违约即验收红）

1. 准入三重门，缺一即拒：访侧本地监听只绑 `127.0.0.1` 字面量（禁 localhost/
   0.0.0.0/::1——cookie 只看 host 不看 port）；被访侧目标白名单显式配置且
   `127.0.0.1:<port>` 精确匹配，默认空 = 全拒；按次开启（会话态，默认关闭，
   不持久化，重启回落关闭）。
2. 反代行为：重写 `Host` 头为目标 `127.0.0.1:<port>`；`Origin`/`Referer` 存在且 host
   为回环字面量时重写为同一目标 authority（无头不造头，非回环/`null` 原样保留），
   其余 method/path/headers/body 原样过隧道；响应与 body 流式转发，禁止整包缓冲
   （规范页 §3.3）。
3. `tunnel_open_dsh` 的 url host 非字面量 `127.0.0.1` / 缺 token / 端口非法 →
   Err 可读中文，且不得先开监听或开流再失败（先校验后动作）。
4. `token` 仅作 URL 透传与 open_url 拼装，与隧道票据鉴权无关（票据身份=底座握手
   PeerId，nonce 才是票据字段）；GUI MUST NOT 把 token 写入任何日志或事件载荷。
5. 审计字段与规范页 §5 闭集逐字对应（snake_case→camelCase 映射：session_id→
   sessionId 等八字段全量），outcome 取值 `"open"`/`"ok"`/六值错误码，GUI 不得
   自造第四类取值。
6. 被访侧服务面准入语义（规范页 §5.2 冻结）：enabled 默认关闭、白名单默认空 =
   全拒、目标 `127.0.0.1:<port>` 精确匹配；`tunnel_serve_start`/`tunnel_serve_stop`
   与每次拒绝都必须可观测（emit tunnel_status 或 Err 可读中文），禁止静默。
7. 持久化口径：`allow` 累积项持久化（随 GUI 配置存盘，重启保留）；`enabled` 为
   会话态不持久化，重启回落 false——即重启后白名单仍在但全拒，直到显式开启
   （「按次开启」语义，对齐规范页 §5.2）。
8. CLI 对等：四条命令登记 cli-parity.tsv exempt——访侧会话与本地反代生命周期绑定
   GUI 进程内（Tauri 事件 emit + 系统浏览器打开），被访侧 enabled 为不持久化的
   会话态，均无 CLI 常驻进程面可对等（llm_share_serve_status 先例）；登记随命令
   落地分卡进行（serve 两条随 W-T2，open_dsh/status 随 W-T3）。headless 隧道场景
   如出现真实需求，另立卡评估 `p2pctl tunnel` 子命令后再转 mapped。
