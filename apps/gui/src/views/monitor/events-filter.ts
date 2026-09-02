import { describeNodeEvent } from "@/lib/event-text";
import type { NodeEventJson, NodeEventType } from "@/lib/ipc-types";
import { isNodeEventError } from "./event-meta";

export interface EventsFilterOptions {
  query: string;
  errorOnly: boolean;
  typeFilter: ReadonlySet<NodeEventType>;
}

// 类型多选 + 仅错误 + 文本搜索（命中原始负载摘要）。
export function filterEvents(
  events: NodeEventJson[],
  options: EventsFilterOptions,
): NodeEventJson[] {
  const q = options.query.trim().toLowerCase();
  return events.filter(
    (event) =>
      options.typeFilter.has(event.type) &&
      (!options.errorOnly || isNodeEventError(event)) &&
      (q.length === 0 ||
        describeNodeEvent(event).toLowerCase().includes(q)),
  );
}
