import { describe, expect, it } from "vitest";

import type { PeerEntry } from "./event-reducer";
import { selectPeerCount, selectPeerList } from "./node-store";

function entry(peerId: string, lastSeenMs: number): PeerEntry {
  return { peerId, addrs: [], connected: true, lastSeenMs, hops: [] };
}

function stateWith(peers: Record<string, PeerEntry>) {
  return { peers } as unknown as Parameters<typeof selectPeerList>[0];
}

// 回归：selectPeerList 曾每次调用返回新数组，useSyncExternalStore 快照不稳定
// 导致 CommandPalette/peers 视图无限重渲染崩 ErrorBoundary（gui-agent errors 实证）。
describe("selectPeerList 快照稳定性", () => {
  it("同一 peers 引用返回同一数组（getSnapshot 缓存）", () => {
    const state = stateWith({ a: entry("a", 1), b: entry("b", 2) });
    expect(selectPeerList(state)).toBe(selectPeerList(state));
  });

  it("peers 引用变化才重算，并按 lastSeenMs 降序", () => {
    const state = stateWith({ a: entry("a", 1), b: entry("b", 2) });
    const first = selectPeerList(state);
    expect(first.map((p) => p.peerId)).toEqual(["b", "a"]);
    const changed = stateWith({ ...state.peers, c: entry("c", 3) });
    const second = selectPeerList(changed);
    expect(second).not.toBe(first);
    expect(second.map((p) => p.peerId)).toEqual(["c", "b", "a"]);
  });

  it("selectPeerCount 返回基元值，天然稳定", () => {
    const state = stateWith({ a: entry("a", 1) });
    expect(selectPeerCount(state)).toBe(1);
  });
});