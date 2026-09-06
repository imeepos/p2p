import type { NodeEventJson, NodeEventType } from "@/lib/ipc-types";
import type { I18nKey } from "@/i18n/types";
import { ALL_EVENT_TYPES } from "@/views/network/event-meta";

export interface EventFilterGroup {
  id: string;
  labelKey: I18nKey;
  types: readonly NodeEventType[];
}

// F20：17 个类型按域五组（连接/消息/群组/安全/节点），数组顺序即展示顺序。
// 安全组恰好覆盖全部错误类型（event-meta ERROR_TYPES），语义同源。
export const EVENT_FILTER_GROUPS: readonly EventFilterGroup[] = [
  {
    id: "connection",
    labelKey: "uxiEvents.filter.group.connection",
    types: ["peer_discovered", "peer_connected", "peer_disconnected", "dial_hop"],
  },
  {
    id: "message",
    labelKey: "uxiEvents.filter.group.message",
    types: ["chat_message", "chat_status", "chat_invite"],
  },
  {
    id: "group",
    labelKey: "uxiEvents.filter.group.group",
    types: [
      "chat_group_message",
      "chat_group_status",
      "chat_group_state",
      "chat_group_invite",
    ],
  },
  {
    id: "security",
    labelKey: "uxiEvents.filter.group.security",
    types: ["listen_failed", "dial_failed", "protocol_violation", "node_error"],
  },
  {
    id: "node",
    labelKey: "uxiEvents.filter.group.node",
    types: ["node_started", "node_stopped"],
  },
];

// chip 命中计数：统计整个缓冲区按类型的条数，不受当前筛选影响；
// 空缓冲返回全 0，保证「0 计数可见」。
export function countEventsByType(
  events: NodeEventJson[],
): Record<NodeEventType, number> {
  const counts = Object.create(null) as Record<NodeEventType, number>;
  for (const type of ALL_EVENT_TYPES) counts[type] = 0;
  for (const event of events) counts[event.type] += 1;
  return counts;
}
