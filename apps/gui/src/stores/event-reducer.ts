import type { DialHopJson, NodeEventJson, NodeStatus, PeerSource } from "@/lib/ipc-types";

export interface PeerEntry {
  peerId: string;
  addrs: string[];
  /** 契约 v5：地址簿聚合来源（mdns > rendezvous > manual），随 peer_discovered 更新。 */
  source: PeerSource;
  connected: boolean;
  /** 发现面最后确认时刻：仅发现源消息与连接成功刷新（语义见 peersAfterEvent）。 */
  lastSeenMs: number;
  hops: DialHopJson[];
}

export interface EventStateSlice {
  events: NodeEventJson[];
  peers: Record<string, PeerEntry>;
  status: NodeStatus | null;
  /**
   * 事件累计序号：单调递增，环形缓冲淘汰旧事件后增量计数仍精确。
   * 可选仅为兼容既有第三方 fixture（省略按 0 起算）；store 初始态与
   * reduceEvent 返回值恒携带该字段，消费方读到的永远是 number。
   */
  eventSeq?: number;
}

export const MAX_EVENTS = 1000;
export const MAX_HOPS_PER_PEER = 20;

function emptyPeer(peerId: string): PeerEntry {
  return { peerId, addrs: [], source: "manual", connected: false, lastSeenMs: 0, hops: [] };
}

function patchPeer(
  peers: Record<string, PeerEntry>,
  peerId: string,
  patch: Partial<PeerEntry>,
): Record<string, PeerEntry> {
  const entry = peers[peerId] ?? emptyPeer(peerId);
  return {
    ...peers,
    [peerId]: { ...entry, ...patch },
  };
}

// 活跃度语义（拆分“地址新鲜”与“可连通”，2026-09-03 邻居表复盘）：
// - lastSeenMs 只由正向证据刷新：发现源（mdns/rendezvous）消息与连接成功。
//   manual 来源的 peer_discovered 是本端自身登记，不证明对端存活；
//   dial_hop 成败都可能出现，不构成对端在线证据；peer_disconnected 可能来自
//   发现缓存 TTL 过期（swarm on_peer_expired），同样不是正向证据。
// - connected 由 connected/disconnected 事件翻转。
function peersAfterEvent(
  peers: Record<string, PeerEntry>,
  event: NodeEventJson,
): Record<string, PeerEntry> {
  switch (event.type) {
    case "peer_discovered": {
      const fresh = event.source !== "manual";
      return patchPeer(peers, event.peer, {
        addrs: event.addrs,
        source: event.source,
        ...(fresh ? { lastSeenMs: Date.now() } : {}),
      });
    }
    case "peer_connected":
      return patchPeer(peers, event.peer, { connected: true, lastSeenMs: Date.now() });
    case "peer_disconnected":
      return patchPeer(peers, event.peer, { connected: false });
    case "dial_hop": {
      const entry = peers[event.peer] ?? emptyPeer(event.peer);
      return patchPeer(peers, event.peer, {
        hops: [
          { hop: event.hop, ok: event.ok, detail: event.detail },
          ...entry.hops,
        ].slice(0, MAX_HOPS_PER_PEER),
      });
    }
    default:
      return peers;
  }
}

function statusAfterEvent(
  status: NodeStatus | null,
  event: NodeEventJson,
): NodeStatus | null {
  if (!status) return status;
  if (event.type === "node_started") {
    return { ...status, running: true, listenAddrs: event.listenAddrs };
  }
  if (event.type === "node_stopped") {
    return {
      ...status,
      running: false,
      listenAddrs: [],
      uptimeSecs: 0,
      startedAtMs: null,
    };
  }
  return status;
}

export function reduceEvent(
  state: EventStateSlice,
  event: NodeEventJson,
): EventStateSlice {
  return {
    events: [event, ...state.events].slice(0, MAX_EVENTS),
    peers: peersAfterEvent(state.peers, event),
    status: statusAfterEvent(state.status, event),
    eventSeq: (state.eventSeq ?? 0) + 1,
  };
}
