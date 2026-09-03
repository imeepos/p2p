import type { NodeEventJson } from "@/lib/ipc-types";

// 事件一行摘要：视图波次再按 type 做 i18n 文案，骨架期用契约字段直排。
export function describeNodeEvent(event: NodeEventJson): string {
  const short = (peerId: string): string => peerId.slice(0, 8);
  switch (event.type) {
    case "peer_discovered":
      return "peer_discovered " + short(event.peer) + " " + (event.addrs[0] ?? "");
    case "peer_connected":
      return "peer_connected " + short(event.peer);
    case "peer_disconnected":
      return "peer_disconnected " + short(event.peer);
    case "listen_failed":
      return "listen_failed " + event.addr + ": " + event.reason;
    case "dial_failed":
      return "dial_failed " + (event.peer ? short(event.peer) : "-") + ": " + event.reason;
    case "protocol_violation":
      return "protocol_violation " + short(event.peer) + ": " + event.reason;
    case "dial_hop":
      return (
        "dial_hop " + short(event.peer) + " " + event.hop + " " +
        (event.ok ? "ok " : "fail ") + event.detail
      );
    case "node_started":
      return "node_started " + event.listenAddrs.join(", ");
    case "node_stopped":
      return "node_stopped";
    case "node_error":
      return "node_error " + event.reason;
    case "chat_message":
      return "chat_message " + short(event.peer);
    case "chat_status":
      return "chat_status " + short(event.peer) + " " + event.status;
  }
}
