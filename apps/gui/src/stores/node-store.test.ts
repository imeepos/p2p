import { describe, expect, it } from "vitest";

import {
  selectPeerCount,
  selectPeerList,
  type NodeStoreState,
} from "./node-store";
import type { PeerEntry } from "./event-reducer";

const peer = (id: string, lastSeenMs: number): PeerEntry => ({
  peerId: id,
  addrs: [],
  connected: true,
  lastSeenMs,
  hops: [],
});

const state = (peers: Record<string, PeerEntry>): NodeStoreState =>
  ({ peers } as unknown as NodeStoreState);

// 回归：useSyncExternalStore 按引用比较快照，selector 必须返回稳定引用。
describe("node-store selectors", () => {
  it("selectPeerList 同一 peers 引用返回同一数组（快照稳定）", () => {
    const s = state({ a: peer("a", 1) });
    expect(selectPeerList(s)).toBe(selectPeerList(s));
  });

  it("selectPeerList peers 变化时返回新数组且按 lastSeenMs 降序", () => {
    const s1 = state({ a: peer("a", 1), b: peer("b", 9) });
    const list1 = selectPeerList(s1);
    expect(list1.map((p) => p.peerId)).toEqual(["b", "a"]);

    const s2 = state({ a: peer("a", 5) });
    const list2 = selectPeerList(s2);
    expect(list2).not.toBe(list1);
    expect(list2.map((p) => p.peerId)).toEqual(["a"]);
  });

  it("selectPeerList 空 peers 引用稳定且计数为零", () => {
    const s = state({});
    expect(selectPeerList(s)).toBe(selectPeerList(s));
    expect(selectPeerList(s)).toHaveLength(0);
    expect(selectPeerCount(s)).toBe(0);
  });
});
