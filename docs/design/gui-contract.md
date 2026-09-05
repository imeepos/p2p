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
互通属后续协议扩展，不在本契约范围内。与 GuiConfig 完全独立：不进 node_start，
保存后无需重启节点即生效。

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
GUI 列表按组分节展示、未分组虚拟组置底，CLI friends --group 同卡对齐）。
实时通话/群聊/已读回执不在本轮。底座只读，全部落 crates/p2p-chat + src-tauri 消费面。

### 12.1 命令表（追加，全部 camelCase；参数无效一律 Err 可读中文）

| 命令 | 参数 | 返回 | 语义 |
|---|---|---|---|
| chat_friends_list | - | ChatFriendJson[] | 读好友簿（无文件返回空数组） |
| chat_friend_add | peerId: string, nickname: string, addrs: string[] | ChatFriendJson | 校验（peerId base58 且 ≠ 本机、nickname trim ≤64、addr 语法逐条校验）后原子写好友簿；addr 同时登记地址簿可拨 |
| chat_friend_remove | peerId: string | boolean | 从好友簿移除；never 在簿 → false（幂等），不删消息历史 |
| chat_friend_update | peerId: string, patch: { group?: string \| null; nickname?: string \| null; note?: string \| null } | ChatFriendJson | 资料补丁（IM-T43 加法）：group/nickname/note 至少一项，addrs 不可经此修改；group 空串 = 移出分组（归一化 null，不落盘空串）；组名 trim 后 ≤32 字符；空补丁或 peer 不在簿 → Err |
| chat_history | peer: string, beforeId?: string | null, limit?: number | ChatMessageJson[] | 按 time desc 分页，limit 默认 50 上限 100；beforeId 游标=严格更早 |
| chat_send | peer: string, kind: ChatKind, text?: string, media?: ChatMediaInput, replyTo?: string \| null | ChatSendReport | 校验→生成信封→落 outbox→尝试发送；文本 trim 后 1..=2000 字符；媒体原始字节 ≤64MiB；replyTo 提供时须非空字符串（不校验被引用消息存在性，离线引用允许） |
| chat_media_file | peer: string, messageId: string | { path: string; mime: string; name: string } | 返回附件落盘绝对路径（仅本端展示用）；消息非 media 或不存在 → Err |
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
  名称/大小并提供下载锚点。系统级"打开默认应用"不在本轮契约内。
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


