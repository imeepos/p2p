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
旧端忽略未知字段照常收信，不校验被引用消息存在性——离线引用允许）。
实时通话/群聊/已读回执不在本轮。底座只读，全部落 crates/p2p-chat + src-tauri 消费面。

### 12.1 命令表（追加，全部 camelCase；参数无效一律 Err 可读中文）

| 命令 | 参数 | 返回 | 语义 |
|---|---|---|---|
| chat_friends_list | - | ChatFriendJson[] | 读好友簿（无文件返回空数组） |
| chat_friend_add | peerId: string, nickname: string, addrs: string[] | ChatFriendJson | 校验（peerId base58 且 ≠ 本机、nickname trim ≤64、addr 语法逐条校验）后原子写好友簿；addr 同时登记地址簿可拨 |
| chat_friend_remove | peerId: string | boolean | 从好友簿移除；never 在簿 → false（幂等），不删消息历史 |
| chat_history | peer: string, beforeId?: string | null, limit?: number | ChatMessageJson[] | 按 time desc 分页，limit 默认 50 上限 100；beforeId 游标=严格更早 |
| chat_send | peer: string, kind: ChatKind, text?: string, media?: ChatMediaInput, replyTo?: string \| null | ChatSendReport | 校验→生成信封→落 outbox→尝试发送；文本 trim 后 1..=2000 字符；媒体原始字节 ≤64MiB；replyTo 提供时须非空字符串（不校验被引用消息存在性，离线引用允许） |
| chat_media_file | peer: string, messageId: string | { path: string; mime: string; name: string } | 返回附件落盘绝对路径（仅本端展示用）；消息非 media 或不存在 → Err |

### 12.2 事件（追加到 NodeEventJson 判别联合）

```ts
| { type: "chat_message"; peer: string; message: ChatMessageJson }   // 入站新消息（已落盘）
| { type: "chat_status"; peer: string; messageId: string; status: "pending"|"sent"|"delivered"|"failed" }
```

### 12.3 数据类型

```ts
type ChatKind = "text" | "image" | "audio" | "video" | "file";

interface ChatFriendJson {
  peerId: string;        // base58
  nickname: string;      // trim 后 ≤64；空串回退 PeerId 缩略
  addrs: string[];       // ip/u端口 = QUIC，ip/t端口 = TCP（对齐 §6 语法）
  note?: string | null;
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
}
```

- 持久化位置：`<dataDir>/chat/`（friends.json / outbox/<peer>.jsonl /
  messages/<peer>.jsonl / media/<peer>/<msgId>_<sanitizedName>），介质权限与原子写对齐 §11 纪律。
- 媒体预览：`chat_media_file` 返回 path 后，前端经 Tauri asset protocol（assetProtocol
  scope 须含 chat/media 目录，src-tauri 侧接线）内联展示 image/audio/video；file 展示
  名称/大小并提供下载锚点。系统级"打开默认应用"不在本轮契约内。
- 验收对齐点：A 侧 serde 字段名与上表逐字一致（camelCase，Option 序列化 null）；
  B 侧 TS 类型与上表逐字一致；mock 与真实实现同签名。


