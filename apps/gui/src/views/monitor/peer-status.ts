import type { PeerEntry } from "@/stores/node-store";

export type PeerStatusKind = "connected" | "discovered" | "offline";
export type PeerSourceKind = "manual" | "discovered";

const DISCOVERED_FRESH_MS = 10 * 60 * 1000;

export function peerStatusKind(peer: PeerEntry, now: number): PeerStatusKind {
  if (peer.connected) return "connected";
  if (peer.lastSeenMs > 0 && now - peer.lastSeenMs < DISCOVERED_FRESH_MS) {
    return "discovered";
  }
  return "offline";
}

// 契约暂无来源字段：有 dial_hop 记录视为本端手动拨号，否则视为发现获知。
// mdns/rendezvous 细分待契约补 source 字段后接入（缺口已报协调会话）。
export function peerSourceKind(peer: PeerEntry): PeerSourceKind {
  return peer.hops.length > 0 ? "manual" : "discovered";
}
