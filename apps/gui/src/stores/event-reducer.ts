import type { DialHopJson, NodeEventJson, NodeStatus } from "@/lib/ipc-types";

export interface PeerEntry {
  peerId: string;
  addrs: string[];
  connected: boolean;
  lastSeenMs: number;
  hops: DialHopJson[];
}

export interface EventStateSlice {
  events: NodeEventJson[];
  peers: Record<string, PeerEntry>;
  status: NodeStatus | null;
}

export const MAX_EVENTS = 1000;
export const MAX_HOPS_PER_PEER = 20;

function emptyPeer(peerId: string): PeerEntry {
  return { peerId, addrs: [], connected: false, lastSeenMs: 0, hops: [] };
}

function touchPeer(
  peers: Record<string, PeerEntry>,
  peerId: string,
  patch: Partial<PeerEntry>,
): Record<string, PeerEntry> {
  const entry = peers[peerId] ?? emptyPeer(peerId);
  return {
    ...peers,
    [peerId]: { ...entry, ...patch, lastSeenMs: Date.now() },
  };
}

function peersAfterEvent(
  peers: Record<string, PeerEntry>,
  event: NodeEventJson,
): Record<string, PeerEntry> {
  switch (event.type) {
    case "peer_discovered":
      return touchPeer(peers, event.peer, { addrs: event.addrs });
    case "peer_connected":
      return touchPeer(peers, event.peer, { connected: true });
    case "peer_disconnected":
      return touchPeer(peers, event.peer, { connected: false });
    case "dial_hop": {
      const entry = peers[event.peer] ?? emptyPeer(event.peer);
      return {
        ...peers,
        [event.peer]: {
          ...entry,
          lastSeenMs: Date.now(),
          hops: [
            { hop: event.hop, ok: event.ok, detail: event.detail },
            ...entry.hops,
          ].slice(0, MAX_HOPS_PER_PEER),
        },
      };
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
  };
}
