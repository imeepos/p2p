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

export type NodeEventJson =
  | { type: "peer_discovered"; peer: string; addrs: string[] }
  | { type: "peer_connected"; peer: string }
  | { type: "peer_disconnected"; peer: string }
  | { type: "listen_failed"; addr: string; reason: string }
  | { type: "dial_failed"; peer: string | null; reason: string }
  | { type: "protocol_violation"; peer: string; reason: string }
  | { type: "dial_hop"; peer: string; hop: DialHopKind; ok: boolean; detail: string }
  | { type: "node_started"; listenAddrs: string[] }
  | { type: "node_stopped" }
  | { type: "node_error"; reason: string };

export type NodeEventType = NodeEventJson["type"];

export type NodeEventHandler = (event: NodeEventJson) => void;
export type UnlistenFn = () => void;

export interface IpcBackend {
  nodeStart(cfg: GuiConfig): Promise<NodeStatus>;
  nodeStop(): Promise<NodeStatus>;
  nodeStatus(): Promise<NodeStatus>;
  metricsGet(): Promise<MetricsJson>;
  configGet(): Promise<GuiConfig>;
  configSave(cfg: GuiConfig): Promise<GuiConfig>;
  peerDial(target: string): Promise<DialReport>;
  peerPing(peerId: string, timeoutMs: number): Promise<PingOutcome>;
  identityReset(confirm: boolean): Promise<NodeStatus>;
  onNodeEvent(handler: NodeEventHandler): Promise<UnlistenFn>;
}
