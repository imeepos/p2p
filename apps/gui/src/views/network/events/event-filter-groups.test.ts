import { describe, expect, it } from "vitest";

import type { NodeEventJson } from "@/lib/ipc-types";
import { ALL_EVENT_TYPES } from "@/views/network/event-meta";
import {
  countEventsByType,
  EVENT_FILTER_GROUPS,
} from "./event-filter-groups";

// 五组并集必须恰好覆盖全部事件类型：未来新增类型漏分组时在此暴露。
describe("EVENT_FILTER_GROUPS 覆盖完整性", () => {
  it("并集等于 ALL_EVENT_TYPES 且组间无重复", () => {
    const union = EVENT_FILTER_GROUPS.flatMap((g) => [...g.types]);
    expect(new Set(union).size).toBe(union.length);
    expect([...union].sort()).toEqual([...ALL_EVENT_TYPES].sort());
  });

  it("每个组至少一个类型且 id 唯一", () => {
    const ids = EVENT_FILTER_GROUPS.map((g) => g.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const g of EVENT_FILTER_GROUPS) expect(g.types.length).toBeGreaterThan(0);
  });
});

describe("countEventsByType", () => {
  it("空缓冲返回全 0（0 计数可见）", () => {
    const counts = countEventsByType([]);
    for (const type of ALL_EVENT_TYPES) expect(counts[type]).toBe(0);
  });

  it("按类型正确累加", () => {
    const events: NodeEventJson[] = [
      { type: "peer_connected", peer: "a", tsMs: 1 },
      { type: "peer_connected", peer: "b", tsMs: 2 },
      { type: "node_error", reason: "x", tsMs: 3 },
      { type: "node_started", listenAddrs: [], tsMs: 4 },
    ];
    const counts = countEventsByType(events);
    expect(counts.peer_connected).toBe(2);
    expect(counts.node_error).toBe(1);
    expect(counts.node_started).toBe(1);
    expect(counts.chat_message).toBe(0);
  });
});
