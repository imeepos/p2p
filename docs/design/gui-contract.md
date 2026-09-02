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
| peer_ping | peerId: string, timeoutMs: number | PingOutcome | 复用 echo 协议 node.request（同 CLI ping），返回 rtt 与期间逐跳 |
| identity_reset | confirm: boolean | NodeStatus | 危险：停止节点并删除身份数据目录内种子文件（必须 confirm=true），返回重置后的状态（未运行） |

## 2. 事件通道

后端 `app.emit("node-event", <NodeEventJson>)`；前端 `listen("node-event", ...)` 单例订阅入 store。

NodeEventJson 判别联合（type 字段）；所有变体均可携带可选 `tsMs?: number`（后端发射时刻毫秒时间戳，前端缺省时以本地接收时间兜底，加法字段不破坏既有实现）：

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
  bootstrap: string[];         // rendezvous 地址，语法同 §6："ip/u端口"（QUIC）或 "ip/t端口"（TCP）
  relayAddrs: string[];
  advertisedAddrs: string[];
  observationPort: number | null;
  observationAddrs: string[];
}

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
