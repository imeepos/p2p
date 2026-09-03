import type { PeerSource } from "@/lib/ipc-types";
import type { PeerEntry } from "@/stores/node-store";

export type PeerStatusKind = "connected" | "discovered" | "offline";
export type PeerSourceKind = PeerSource;

const DISCOVERED_FRESH_MS = 10 * 60 * 1000;

export function peerStatusKind(peer: PeerEntry, now: number): PeerStatusKind {
  if (peer.connected) return "connected";
  if (peer.lastSeenMs > 0 && now - peer.lastSeenMs < DISCOVERED_FRESH_MS) {
    return "discovered";
  }
  return "offline";
}

// 契约 v5：来源直读 peer_discovered.source（地址簿聚合 mdns > rendezvous > manual），
// 不再以本端拨号行为（hops）推断。
export function peerSourceKind(peer: PeerEntry): PeerSourceKind {
  return peer.source;
}
