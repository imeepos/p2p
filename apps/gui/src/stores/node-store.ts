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

/** 引导生命周期：error 时界面必须给出显式错误态与重试入口，禁止静默骨架。 */
export type BootstrapPhase = "idle" | "loading" | "ready" | "error";

/** 周期刷新连续失败达该阈值即视为数据可能过期（5s 轮询下约 15s）。 */
export const REFRESH_STALE_THRESHOLD = 3;

function toErrorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export interface NodeStoreState {
  status: NodeStatus | null;
  metrics: MetricsJson | null;
  metricsHistory: MetricsPoint[];
  peers: Record<string, PeerEntry>;
  events: NodeEventJson[];
  eventSeq: number;
  subscriptionLive: boolean;
  bootstrapPhase: BootstrapPhase;
  bootstrapError: string | null;
  dataStale: boolean;
  consecutiveRefreshFailures: number;
  lastRefreshError: string | null;
  bootstrap: () => Promise<void>;
  refresh: () => Promise<boolean>;
  startNode: (cfg: GuiConfig) => Promise<NodeStatus>;
  stopNode: () => Promise<NodeStatus>;
  dial: (target: string) => Promise<DialReport>;
  connect: (peerId: string) => Promise<DialReport>;
  disconnect: (peerId: string) => Promise<boolean>;
  ping: (peerId: string, timeoutMs: number) => Promise<PingOutcome>;
}

export const useNodeStore = create<NodeStoreState>()((set, get) => ({
  status: null,
  metrics: null,
  metricsHistory: [],
  peers: {},
  events: [],
  eventSeq: 0,
  subscriptionLive: false,
  bootstrapPhase: "idle",
  bootstrapError: null,
  dataStale: false,
  consecutiveRefreshFailures: 0,
  lastRefreshError: null,

  bootstrap: async () => {
    const phase = get().bootstrapPhase;
    // loading 防并发进入，ready 幂等；error 允许再次执行——旧的一次性
    // 幂等锁（模块级 subscriptionStarted）在订阅/刷新失败后整会话不再
    // 重试，界面永挂骨架，是 P0 缺陷根源。
    if (phase === "loading" || phase === "ready") return;
    set({ bootstrapPhase: "loading", bootstrapError: null });
    if (!get().subscriptionLive) {
      try {
        const unlisten = await ipc.onNodeEvent((event) =>
          set((s) => reduceEvent(s, event)),
        );
        void unlisten;
        set({ subscriptionLive: true });
      } catch (error) {
        console.error("[node-store] 事件订阅失败", error);
        set({ bootstrapPhase: "error", bootstrapError: toErrorText(error) });
        return;
      }
    }
    const ok = await get().refresh();
    if (ok) {
      set({ bootstrapPhase: "ready" });
    } else {
      set({ bootstrapPhase: "error", bootstrapError: get().lastRefreshError });
    }
  },

  refresh: async () => {
    try {
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
        consecutiveRefreshFailures: 0,
        dataStale: false,
        lastRefreshError: null,
      }));
      return true;
    } catch (error) {
      // 不上抛：轮询定时器与引导各自消费返回值；连败达阈值置 dataStale
      // 驱动「数据可能已过期」横幅，恢复成功后自动消失。
      console.error("[node-store] 状态刷新失败", error);
      set((s) => {
        const consecutiveRefreshFailures = s.consecutiveRefreshFailures + 1;
        return {
          consecutiveRefreshFailures,
          dataStale: consecutiveRefreshFailures >= REFRESH_STALE_THRESHOLD,
          lastRefreshError: toErrorText(error),
        };
      });
      return false;
    }
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

// 在线口径：底层连接建立即在线（展示语义），connected 由 peer_connected/disconnected 事件翻转。
export function usePeerOnline(peerId: string): boolean {
  return useNodeStore((s) => s.peers[peerId]?.connected ?? false);
}

export function selectListenPorts(status: NodeStatus | null): number[] {
  if (!status) return [];
  const ports: number[] = [];
  for (const addr of status.listenAddrs) {
    const matched = addr.match(/\/t?(\d+)$/);
    if (matched) ports.push(Number(matched[1]));
  }
  return ports;
}
