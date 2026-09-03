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

interface NodeStoreState {
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
  ping: (peerId, timeoutMs) => ipc.peerPing(peerId, timeoutMs),
}));

// getSnapshot 稳定性：zustand v5（useSyncExternalStore）要求同一 peers 引用返回同一数组，
// 否则 useNodeStore(selectPeerList) 的组件无限重渲染（Maximum update depth exceeded）。
let peerListCache: { source: Record<string, PeerEntry>; list: PeerEntry[] } | null = null;

export const selectPeerList = (s: NodeStoreState): PeerEntry[] => {
  if (peerListCache?.source === s.peers) return peerListCache.list;
  const list = Object.values(s.peers).sort((a, b) => b.lastSeenMs - a.lastSeenMs);
  peerListCache = { source: s.peers, list };
  return list;
};

export const selectPeerCount = (s: NodeStoreState): number =>
  Object.keys(s.peers).length;

export function selectListenPorts(status: NodeStatus | null): number[] {
  if (!status) return [];
  const ports: number[] = [];
  for (const addr of status.listenAddrs) {
    const matched = addr.match(/\/t?(\d+)$/);
    if (matched) ports.push(Number(matched[1]));
  }
  return ports;
}
