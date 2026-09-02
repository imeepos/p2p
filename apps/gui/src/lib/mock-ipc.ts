import type {
  DialHopKind,
  GuiConfig,
  IpcBackend,
  MetricsJson,
  NodeEventJson,
  NodeEventHandler,
  NodeStatus,
  UnlistenFn,
} from "./ipc-types";
import { parseDialTarget } from "./dial-target";

const START_DELAY_MS = 800;
const STOP_DELAY_MS = 300;
const DIAL_BASE_DELAY_MS = 500;
const PING_TIMEOUT_PROBABILITY = 0.1;
const TICK_MS = 2500;
const DISCOVER_PROBABILITY = 0.5;
const CONNECT_PROBABILITY = 0.35;
const DISCONNECT_PROBABILITY = 0.2;
const HOP_PROBABILITY = 0.25;
const NOT_RUNNING = "节点未运行，请先启动节点";

const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

const DEFAULT_CONFIG: GuiConfig = {
  quicPort: 0,
  tcpPort: 0,
  enableMdns: true,
  dataDir: "<app-data>/p2p-data",
  bootstrap: [],
  relayAddrs: [],
  advertisedAddrs: [],
  observationPort: null,
  observationAddrs: [],
};

interface MockState {
  running: boolean;
  startedAtMs: number | null;
  config: GuiConfig;
  listenAddrs: string[];
  peerId: string;
  knownPeers: string[];
  metrics: MetricsJson;
}

const state: MockState = {
  running: false,
  startedAtMs: null,
  config: DEFAULT_CONFIG,
  listenAddrs: [],
  peerId: randomPeerId(),
  knownPeers: [],
  metrics: emptyMetrics(),
};

const handlers = new Set<NodeEventHandler>();
let tickTimer: number | null = null;

function emptyMetrics(): MetricsJson {
  return {
    dialDirectOk: 0, dialDirectFail: 0,
    dialPunchOk: 0, dialPunchFail: 0,
    dialRelayOk: 0, dialRelayFail: 0,
    addrDialFailures: 0,
    relayReconnects: 0,
    gateDenialsTotal: 0,
    activeConnections: 0,
    relaySessionsActive: 0,
  };
}

function randomPeerId(): string {
  let id = "";
  for (let i = 0; i < 44; i += 1) {
    id += B58[Math.floor(Math.random() * B58.length)];
  }
  return id;
}

function randomAddr(): string {
  const ip = "192.168." + Math.floor(Math.random() * 254 + 1) + "." + Math.floor(Math.random() * 254 + 1);
  const port = 30000 + Math.floor(Math.random() * 9999);
  return Math.random() < 0.7 ? ip + "/" + port : ip + "/t" + port;
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

// 对齐真实实现（events::emit）：出口统一盖发射时刻毫秒戳（契约 §2）。
function emit(event: NodeEventJson): void {
  const stamped = { ...event, tsMs: Date.now() };
  handlers.forEach((handler) => handler(stamped));
}

function snapshot(): NodeStatus {
  return {
    running: state.running,
    peerId: state.running ? state.peerId : null,
    listenAddrs: state.running ? state.listenAddrs : [],
    uptimeSecs:
      state.running && state.startedAtMs
        ? Math.floor((Date.now() - state.startedAtMs) / 1000)
        : 0,
    startedAtMs: state.startedAtMs,
    config: state.config,
  };
}

function currentMetrics(): MetricsJson {
  if (!state.running) return emptyMetrics();
  state.metrics.activeConnections = Math.max(
    0,
    state.metrics.activeConnections + Math.round((Math.random() - 0.5) * 2),
  );
  state.metrics.relaySessionsActive = Math.max(
    0,
    Math.round(state.metrics.activeConnections / 3),
  );
  return { ...state.metrics };
}

function startEventStream(): void {
  stopEventStream();
  tickTimer = window.setInterval(() => {
    if (!state.running) return;
    if (Math.random() < DISCOVER_PROBABILITY) {
      const peer = randomPeerId();
      state.knownPeers.push(peer);
      emit({ type: "peer_discovered", peer, addrs: [randomAddr()] });
    }
    if (state.knownPeers.length > 0 && Math.random() < CONNECT_PROBABILITY) {
      emit({ type: "peer_connected", peer: pickKnownPeer() });
    }
    if (state.knownPeers.length > 0 && Math.random() < DISCONNECT_PROBABILITY) {
      emit({ type: "peer_disconnected", peer: pickKnownPeer() });
    }
    if (Math.random() < HOP_PROBABILITY) {
      emitDialHop("relay", Math.random() < 0.85);
    }
  }, TICK_MS);
}

function stopEventStream(): void {
  if (tickTimer !== null) {
    window.clearInterval(tickTimer);
    tickTimer = null;
  }
}

function pickKnownPeer(): string {
  return state.knownPeers[Math.floor(Math.random() * state.knownPeers.length)];
}

function emitDialHop(hop: DialHopKind, ok: boolean): void {
  if (state.knownPeers.length === 0) state.knownPeers.push(randomPeerId());
  if (ok) {
    state.metrics[hop === "direct" ? "dialDirectOk" : hop === "punch" ? "dialPunchOk" : "dialRelayOk"] += 1;
  } else {
    state.metrics[hop === "direct" ? "dialDirectFail" : hop === "punch" ? "dialPunchFail" : "dialRelayFail"] += 1;
  }
  emit({
    type: "dial_hop",
    peer: pickKnownPeer(),
    hop,
    ok,
    detail: ok ? hop + " established (mock)" : hop + " unavailable (mock)",
  });
}

export const mockBackend: IpcBackend = {
  async nodeStart(cfg) {
    if (state.running) throw new Error("节点已在运行");
    await delay(START_DELAY_MS);
    state.config = cfg;
    state.running = true;
    state.startedAtMs = Date.now();
    const quic = cfg.quicPort || 34000 + Math.floor(Math.random() * 500);
    const tcp = cfg.tcpPort || quic + 1;
    state.listenAddrs = ["0.0.0.0/" + quic, "0.0.0.0/t" + tcp];
    emit({ type: "node_started", listenAddrs: state.listenAddrs });
    startEventStream();
    return snapshot();
  },

  // 幂等：仅在真的停掉运行中节点时发 node_stopped（对齐 state::stop + 命令层）。
  async nodeStop() {
    await delay(STOP_DELAY_MS);
    const wasRunning = state.running;
    stopEventStream();
    state.running = false;
    state.startedAtMs = null;
    state.listenAddrs = [];
    state.metrics.activeConnections = 0;
    state.metrics.relaySessionsActive = 0;
    if (wasRunning) emit({ type: "node_stopped" });
    return snapshot();
  },

  async nodeStatus() {
    return snapshot();
  },

  async metricsGet() {
    return currentMetrics();
  },

  async configGet() {
    return { ...state.config };
  },

  async configSave(cfg) {
    await delay(200);
    state.config = cfg;
    return { ...cfg };
  },

  // 契约 §6 语法预检与 proto::parse_target 同规则族（base58 长度/IpAddr/u t/端口段）。
  async peerDial(target) {
    const parsed = parseDialTarget(target);
    if (!parsed) {
      throw new Error(`target 语法非法，应为 <peer_id>@<addr>，实得 "${target}"`);
    }
    if (!state.running) throw new Error(NOT_RUNNING);
    const { peerId: peer, addr } = parsed;
    const startedAt = Date.now();
    await delay(DIAL_BASE_DELAY_MS + Math.random() * 700);
    const hops = [];
    const chain: DialHopKind[] = ["direct", "punch", "relay"];
    for (let i = 0; i < chain.length; i += 1) {
      const ok = i === chain.length - 1 ? true : Math.random() < 0.55;
      const kind = chain[i];
      emit({ type: "dial_hop", peer, hop: kind, ok, detail: kind + " -> " + addr });
      hops.push({ hop: kind, ok, detail: kind + " -> " + addr });
      if (ok) {
        if (!state.knownPeers.includes(peer)) state.knownPeers.push(peer);
        emit({ type: "peer_connected", peer });
        return { peer, hops, ok: true, totalMs: Date.now() - startedAt };
      }
    }
    return { peer, hops, ok: false, totalMs: Date.now() - startedAt };
  },

  // 未知节点与超时都返回失败 outcome（对齐 peers::ping），不抛 IPC 错。
  async peerPing(peerId, timeoutMs) {
    if (timeoutMs === 0) throw new Error("timeoutMs 必须为正数");
    if (!state.running) throw new Error(NOT_RUNNING);
    await delay(Math.min(timeoutMs, 120 + Math.random() * 400));
    if (!state.knownPeers.includes(peerId)) {
      return {
        ok: false,
        rttMs: null,
        hops: [],
        error: "echo 请求失败: 对端不可达或未知（mock）",
      };
    }
    const hops = [{ hop: "direct" as DialHopKind, ok: true, detail: "echo via direct" }];
    if (Math.random() < PING_TIMEOUT_PROBABILITY) {
      return { ok: false, rttMs: null, hops, error: "echo 超时" };
    }
    return { ok: true, rttMs: Math.round(15 + Math.random() * 180), hops, error: null };
  },

  // confirm 校验在命令层（真实实现如此）；node_stopped 仅在停了运行中节点时发。
  async identityReset(confirm) {
    if (!confirm) throw new Error("重置身份必须显式 confirm=true");
    const wasRunning = state.running;
    stopEventStream();
    state.running = false;
    state.startedAtMs = null;
    state.listenAddrs = [];
    state.knownPeers = [];
    state.peerId = randomPeerId();
    state.metrics = emptyMetrics();
    if (wasRunning) emit({ type: "node_stopped" });
    return snapshot();
  },

  onNodeEvent(handler): Promise<UnlistenFn> {
    handlers.add(handler);
    return Promise.resolve(() => {
      handlers.delete(handler);
    });
  },
};
