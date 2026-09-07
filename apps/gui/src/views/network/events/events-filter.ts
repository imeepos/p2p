import { describeNodeEvent } from "@/lib/event-text";
import type { NodeEventJson, NodeEventType } from "@/lib/ipc-types";
import { isNodeEventError } from "@/views/network/event-meta";

export interface EventsFilterOptions {
  query: string;
  errorOnly: boolean;
  typeFilter: ReadonlySet<NodeEventType>;
  /** 展示同源检索文本（N1）：命中界面可见文案（人话摘要+昵称）；
   *  缺省退回原始负载摘要（内部串，用户按所见搜索会零命中）。 */
  searchText?: (event: NodeEventJson) => string;
}

// 类型多选 + 仅错误 + 文本搜索。
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
        (options.searchText?.(event) ?? describeNodeEvent(event))
          .toLowerCase()
          .includes(q)),
  );
}
