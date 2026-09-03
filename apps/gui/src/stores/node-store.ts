import { create } from "zustand";

import { ipc } from "@/lib/ipc";
import type {
  DialReport,
  GuiConfig,
  MetricsJson,
  MetricsPoint,
  NodeEventJson,
  NodeStatus,
  PingOutcome,
} from "@/lib/ipc-types";
import { metricsSnapshotPoint, appendMetricsPoint } from "@/lib/metrics-history";

import { reduceEvent, type PeerEntry } from "./event-reducer";

export type { PeerEntry };

export interface NodeStoreState {
  status: NodeStatus | null;
  metrics: MetricsJson | null;
  metricsHistory: MetricsPoint[];
  peers: Record<string, PeerEntry>;
  events: NodeEventJson[];
  subscriptionLive: boolean;
  bootstrap: () => Promise<void>;
  refresh: () => Promise<void>;
  startNode: (cfg: GuiConfig) => Promise<NodeStatus>;
  stopNode: () => Promise<NodeStatus>;
  dial: (target: string) => Promise<DialReport>;
  connect: (peerId: string) => Promise<DialReport>;
  disconnect: (peerId: string) => Promise<boolean>;
  ping: (peerId: string, timeoutMs: number) => Promise<PingOutcome>;
}

let subscriptionStarted = false;

export const useNodeStore = create<NodeStoreState>()((set, get) => ({
  status: null,
  metrics: null,
  metricsHistory: [],
  peers: {},
  events: [],
  subscriptionLive: false,

  bootstrap: async () => {
    if (subscriptionStarted) return;
    subscriptionStarted = true;
    const unlisten = await ipc.onNodeEvent((event) =>
      set((s) => reduceEvent(s, event)),
    );
    void unlisten;
    set({ subscriptionLive: true });
    await get().refresh();
  },

  refresh: async () => {
    const [status, metrics] = await Promise.all([
      ipc.nodeStatus(),
      ipc.metricsGet(),
    ]);
    // 每次成功取数即追加趋势采样点（环形 120 点，契约 v2 窗口一致）。
    set((s) => ({
      status,
      metrics,
      metricsHistory: appendMetricsPoint(
        s.metricsHistory,
        metricsSnapshotPoint(metrics),
      ),
    }));
  },

  startNode: async (cfg) => {
    const status = await ipc.nodeStart(cfg);
    set({ status });
    return status;
  },

  stopNode: async () => {
    const status = await ipc.nodeStop();
    set({ status });
    return status;
  },

  dial: (target) => ipc.peerDial(target),
  connect: (peerId) => ipc.peerConnect(peerId),
  disconnect: (peerId) => ipc.peerDisconnect(peerId),
  ping: (peerId, timeoutMs) => ipc.peerPing(peerId, timeoutMs),
}));

// useSyncExternalStore 按引用比较快照：peers 不变时必须返回同一数组引用，
// 否则快照永不收敛，React 无限重渲直至崩溃（白屏级启动故障）。
let peerListCache: { peers: NodeStoreState["peers"]; list: PeerEntry[] } | null = null;

// 地址卫生（E5 复盘）：rendezvous 公共池泄漏的 loopback 条目（127.0.0.1/随机
// 端口）跨网永远不可拨，在邻居表只呈现为「离线」噪音；仅剩不可路由地址且
// 未连接的对端整行隐藏。地址格式与后端一致：`<ip>/u<端口>` 或 `<ip>/t<端口>`。
export function hasDialableAddr(addr: string): boolean {
  const host = addr.split("/")[0]?.toLowerCase() ?? "";
  if (host.length === 0 || host === "localhost" || host === "::1") return false;
  if (/^127(\.\d{1,3}){3}$/.test(host)) return false;
  if (/^fe[89ab]/.test(host)) return false; // fe80::/10 链路本地
  return true;
}

const isVisiblePeer = (p: PeerEntry): boolean =>
  p.connected || p.addrs.some(hasDialableAddr);

export const selectPeerList = (s: NodeStoreState): PeerEntry[] => {
  if (peerListCache === null || peerListCache.peers !== s.peers) {
    peerListCache = {
      peers: s.peers,
      list: Object.values(s.peers)
        .filter(isVisiblePeer)
        .sort((a, b) => b.lastSeenMs - a.lastSeenMs),
    };
  }
  return peerListCache.list;
};

export const selectPeerCount = (s: NodeStoreState): number =>
  selectPeerList(s).length;

export function selectListenPorts(status: NodeStatus | null): number[] {
  if (!status) return [];
  const ports: number[] = [];
  for (const addr of status.listenAddrs) {
    const matched = addr.match(/\/t?(\d+)$/);
    if (matched) ports.push(Number(matched[1]));
  }
  return ports;
}
