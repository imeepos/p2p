// 与 docs/design/gui-contract.md v1 逐字对齐，禁止私自改名；缺口走协调会话加法修订。

export interface GuiConfig {
  quicPort: number; // 0 = 随机
  tcpPort: number; // 0 = 随机
  enableMdns: boolean;
  dataDir: string;
  bootstrap: string[];
  relayAddrs: string[];
  advertisedAddrs: string[];
  observationPort: number | null;
  observationAddrs: string[];
}

export interface NodeStatus {
  running: boolean;
  peerId: string | null; // base58(sha256(pubkey))
  listenAddrs: string[];
  uptimeSecs: number;
  startedAtMs: number | null;
  config: GuiConfig; // 运行中节点的生效配置；未运行回持久化配置
}

export interface MetricsJson {
  dialDirectOk: number;
  dialDirectFail: number;
  dialPunchOk: number;
  dialPunchFail: number;
  dialRelayOk: number;
  dialRelayFail: number;
  addrDialFailures: number;
  relayReconnects: number;
  gateDenialsTotal: number;
  activeConnections: number;
  relaySessionsActive: number;
}

export type DialHopKind = "direct" | "punch" | "relay";

// 契约 v5 加法：peer_discovered.source，地址簿聚合来源（mdns > rendezvous > manual）。
export type PeerSource = "mdns" | "rendezvous" | "manual";

export interface DialHopJson {
  hop: DialHopKind;
  ok: boolean;
  detail: string;
}

export interface DialReport {
  peer: string;
  hops: DialHopJson[];
  ok: boolean;
  totalMs: number;
}

export interface PingOutcome {
  ok: boolean;
  rttMs: number | null;
  hops: DialHopJson[];
  error: string | null;
}

// 契约 v2：后端每 5s 采样，最近 120 点（10 分钟窗口）。
export interface MetricsPoint {
  tMs: number;
  activeConnections: number;
  relaySessionsActive: number;
  dialOkTotal: number;
  dialFailTotal: number;
}

// 契约 §2 修订：全变体可带可选 tsMs（后端 emit 出口统一盖发射时刻戳，缺省字段不出现在载荷）。
export type NodeEventJson =
  | { type: "peer_discovered"; peer: string; addrs: string[]; source: PeerSource; tsMs?: number }
  | { type: "peer_connected"; peer: string; tsMs?: number }
  | { type: "peer_disconnected"; peer: string; tsMs?: number }
  | { type: "listen_failed"; addr: string; reason: string; tsMs?: number }
  | { type: "dial_failed"; peer: string | null; reason: string; tsMs?: number }
  | { type: "protocol_violation"; peer: string; reason: string; tsMs?: number }
  | { type: "dial_hop"; peer: string; hop: DialHopKind; ok: boolean; detail: string; tsMs?: number }
  | { type: "node_started"; listenAddrs: string[]; tsMs?: number }
  | { type: "node_stopped"; tsMs?: number }
  | { type: "node_error"; reason: string; tsMs?: number };

export type NodeEventType = NodeEventJson["type"];

export type NodeEventHandler = (event: NodeEventJson) => void;
export type UnlistenFn = () => void;

// 契约 v4 §9 加法（G-U1/G-U2）：与 docs/design/gui-contract.md 逐字对齐，禁止改名。
export interface UpdateCheckResult {
  currentVersion: string; // 应用当前版本（tauri.conf version）
  latestVersion: string | null; // 无候选时 null
  hasUpdate: boolean;
  releaseUrl: string | null; // release html_url
  releaseName: string | null;
  releaseNotesMd: string | null; // release body 原文
  publishedAtMs: number | null;
  checkedAtMs: number;
}

// 契约 v6 §11 加法：本机节点资料（纯展示，仅存本机，不随发现协议广播）。
export interface NodeProfile {
  name: string; // trim 后 ≤64 字符；空串 = 未命名
  description: string; // ≤280 字符；可空
  avatar: string | null; // data URL（png/jpeg/webp base64，总长 ≤200_000）；null = 未设置
}

export interface IpcBackend {
  nodeStart(cfg: GuiConfig): Promise<NodeStatus>;
  nodeStop(): Promise<NodeStatus>;
  nodeStatus(): Promise<NodeStatus>;
  metricsGet(): Promise<MetricsJson>;
  metricsHistory(): Promise<MetricsPoint[]>;
  configGet(): Promise<GuiConfig>;
  configSave(cfg: GuiConfig): Promise<GuiConfig>;
  peerDial(target: string): Promise<DialReport>;
  peerConnect(peerId: string): Promise<DialReport>;
  peerDisconnect(peerId: string): Promise<boolean>;
  peerPing(peerId: string, timeoutMs: number): Promise<PingOutcome>;
  identityReset(confirm: boolean): Promise<NodeStatus>;
  profileGet(): Promise<NodeProfile>;
  profileSave(profile: NodeProfile): Promise<NodeProfile>;
  updateCheck(): Promise<UpdateCheckResult>;
  updateOpenReleasePage(url: string): Promise<void>;
  onNodeEvent(handler: NodeEventHandler): Promise<UnlistenFn>;
}

// 契约 v3 加法（G-H 观测）：诊断命令面，与节点控制面分离；mock/tauri 同签名。
export interface DiagBackend {
  logPath(): Promise<string>;
  logTail(maxLines: number): Promise<string[]>;
  logClear(): Promise<void>;
}
