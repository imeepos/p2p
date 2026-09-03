import type {
  DialHopKind,
  NodeEventJson,
  NodeEventType,
} from "@/lib/ipc-types";
import type { I18nKey } from "@/i18n/types";

export type BadgeVariant =
  | "default"
  | "secondary"
  | "destructive"
  | "outline";

const ERROR_TYPES: ReadonlySet<NodeEventType> = new Set<NodeEventType>([
  "listen_failed",
  "dial_failed",
  "protocol_violation",
  "node_error",
]);

const BADGE_VARIANT: Record<NodeEventType, BadgeVariant> = {
  peer_discovered: "secondary",
  peer_connected: "default",
  peer_disconnected: "outline",
  listen_failed: "destructive",
  dial_failed: "destructive",
  protocol_violation: "destructive",
  dial_hop: "outline",
  node_started: "secondary",
  node_stopped: "secondary",
  node_error: "destructive",
  chat_message: "default",
  chat_status: "secondary",
};

export const ALL_EVENT_TYPES: readonly NodeEventType[] = Object.keys(
  BADGE_VARIANT,
) as NodeEventType[];

export const EVENT_TYPE_KEY: Record<NodeEventType, I18nKey> = {
  peer_discovered: "events.types.peer_discovered",
  peer_connected: "events.types.peer_connected",
  peer_disconnected: "events.types.peer_disconnected",
  listen_failed: "events.types.listen_failed",
  dial_failed: "events.types.dial_failed",
  protocol_violation: "events.types.protocol_violation",
  dial_hop: "events.types.dial_hop",
  node_started: "events.types.node_started",
  node_stopped: "events.types.node_stopped",
  node_error: "events.types.node_error",
  chat_message: "events.types.chat_message",
  chat_status: "events.types.chat_status",
};

export function isNodeEventError(event: NodeEventJson): boolean {
  return ERROR_TYPES.has(event.type);
}

export function eventBadgeVariant(event: NodeEventJson): BadgeVariant {
  return BADGE_VARIANT[event.type];
}

export interface EventSummary {
  key: I18nKey;
  values: Record<string, string>;
}

const short = (peerId: string | null): string =>
  peerId ? peerId.slice(0, 8) : "-";

type HopLabel = (kind: DialHopKind) => string;

export interface SummaryLabels {
  hopLabel: HopLabel;
  okLabel: string;
  failLabel: string;
}

export function eventSummary(
  event: NodeEventJson,
  labels: SummaryLabels,
): EventSummary {
  switch (event.type) {
    case "peer_discovered":
      return {
        key: "events.summary.peerDiscovered",
        values: { peer: short(event.peer), addr: event.addrs[0] ?? "-" },
      };
    case "peer_connected":
      return {
        key: "events.summary.peerConnected",
        values: { peer: short(event.peer) },
      };
    case "peer_disconnected":
      return {
        key: "events.summary.peerDisconnected",
        values: { peer: short(event.peer) },
      };
    case "listen_failed":
      return {
        key: "events.summary.listenFailed",
        values: { addr: event.addr, reason: event.reason },
      };
    case "dial_failed":
      return {
        key: "events.summary.dialFailed",
        values: { peer: short(event.peer), reason: event.reason },
      };
    case "protocol_violation":
      return {
        key: "events.summary.protocolViolation",
        values: { peer: short(event.peer), reason: event.reason },
      };
    case "dial_hop":
      return {
        key: "events.summary.dialHop",
        values: {
          peer: short(event.peer),
          hop: labels.hopLabel(event.hop),
          outcome: event.ok ? labels.okLabel : labels.failLabel,
          detail: event.detail,
        },
      };
    case "node_started":
      return {
        key: "events.summary.nodeStarted",
        values: { addrs: event.listenAddrs.join(", ") },
      };
    case "node_stopped":
      return { key: "events.summary.nodeStopped", values: {} };
    case "node_error":
      return {
        key: "events.summary.nodeError",
        values: { reason: event.reason },
      };
    case "chat_message":
      return { key: "events.summary.chatMessage", values: { peer: short(event.peer) } };
    case "chat_status":
      return { key: "events.summary.chatStatus", values: { id: event.messageId.slice(0, 8), status: event.status } };
  }
}
