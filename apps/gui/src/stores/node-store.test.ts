import { describe, expect, it } from "vitest";

import {
  hasDialableAddr,
  selectPeerCount,
  selectPeerList,
  type NodeStoreState,
} from "./node-store";
import type { PeerEntry } from "./event-reducer";

const peer = (id: string, lastSeenMs: number): PeerEntry => ({
  peerId: id,
  addrs: [],
  source: "rendezvous",
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

  it("仅剩 loopback 地址的离线对端被隐藏（E5：rendezvous 泄漏废条目）", () => {
    const junk = { ...peer("junk", 3), connected: false, addrs: ["127.0.0.1/u60736", "::1/u40000"] };
    const lan = { ...peer("lan", 2), connected: false, addrs: ["192.168.1.5/u40001"] };
    const live = { ...peer("live", 1), connected: true, addrs: ["127.0.0.1/u40002"] };
    const s = state({ junk, lan, live });
    expect(selectPeerList(s).map((p) => p.peerId)).toEqual(["lan", "live"]);
    expect(selectPeerCount(s)).toBe(2);
  });

  it("hasDialableAddr 覆盖 loopback/链路本地/私网/公网", () => {
    expect(hasDialableAddr("127.0.0.1/u60736")).toBe(false);
    expect(hasDialableAddr("::1/u40000")).toBe(false);
    expect(hasDialableAddr("fe80::1/u40001")).toBe(false);
    expect(hasDialableAddr("localhost/u40002")).toBe(false);
    expect(hasDialableAddr("/u40003")).toBe(false);
    expect(hasDialableAddr("192.168.1.5/u40004")).toBe(true);
    expect(hasDialableAddr("203.0.113.7/t40005")).toBe(true);
    expect(hasDialableAddr("240e:abcd::1/u40006")).toBe(true);
  });
});
