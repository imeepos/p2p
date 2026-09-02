import { create } from "zustand";

import { ipc } from "@/lib/ipc";
import type {
  DialHopJson,
  DialReport,
  GuiConfig,
  MetricsJson,
  NodeEventJson,
  NodeStatus,
  PingOutcome,
} from "@/lib/ipc-types";

export interface PeerEntry {
  peerId: string;
  addrs: string[];
  connected: boolean;
  lastSeenMs: number;
  hops: DialHopJson[];
}

interface NodeStoreState {
  status: NodeStatus | null;
  metrics: MetricsJson | null;
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

const MAX_EVENTS = 500;
const MAX_HOPS_PER_PEER = 20;

type SetState = (
  partial:
    | Partial<NodeStoreState>
    | ((state: NodeStoreState) => Partial<NodeStoreState>),
) => void;

let subscriptionStarted = false;

function touchPeer(
  peers: Record<string, PeerEntry>,
  peerId: string,
  patch: Partial<PeerEntry>,
): Record<string, PeerEntry> {
  const entry =
    peers[peerId] ??
    { peerId, addrs: [], connected: false, lastSeenMs: 0, hops: [] };
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
      const entry =
        peers[event.peer] ??
        { peerId: event.peer, addrs: [], connected: false, lastSeenMs: 0, hops: [] };
      return {
        ...peers,
        [event.peer]: {
          ...entry,
          lastSeenMs: Date.now(),
          hops: [{ hop: event.hop, ok: event.ok, detail: event.detail }, ...entry.hops].slice(
            0,
            MAX_HOPS_PER_PEER,
          ),
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
    return { ...status, running: false, listenAddrs: [], uptimeSecs: 0, startedAtMs: null };
  }
  return status;
}

function applyEvent(set: SetState, event: NodeEventJson): void {
  set((s) => ({
    events: [event, ...s.events].slice(0, MAX_EVENTS),
    peers: peersAfterEvent(s.peers, event),
    status: statusAfterEvent(s.status, event),
  }));
}

export const useNodeStore = create<NodeStoreState>()((set, get) => ({
  status: null,
  metrics: null,
  peers: {},
  events: [],
  subscriptionLive: false,

  bootstrap: async () => {
    if (subscriptionStarted) return;
    subscriptionStarted = true;
    const unlisten = await ipc.onNodeEvent((event) => applyEvent(set, event));
    void unlisten;
    set({ subscriptionLive: true });
    await get().refresh();
  },

  refresh: async () => {
    const [status, metrics] = await Promise.all([
      ipc.nodeStatus(),
      ipc.metricsGet(),
    ]);
    set({ status, metrics });
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

export const selectPeerList = (s: NodeStoreState): PeerEntry[] =>
  Object.values(s.peers).sort((a, b) => b.lastSeenMs - a.lastSeenMs);

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