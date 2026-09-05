// console status HTTP 客户端契约测试：/reattach 与 /discovery 响应映射、
// 容错折化（不可达/坏形/未知 reason → unavailable 或空清单），绝不抛出。
import { afterEach, describe, expect, it, vi } from "vitest";

import { fetchDiscoveryPeers, queryReattachTicket } from "./console-client";

function stubFetch(payload: () => { ok: boolean; body: unknown } | "throw"): void {
  // "throw" 哨兵而非 Promise.reject：被拒 promise 无人 await 会挂成 unhandled rejection
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => {
      const p = payload();
      if (p === "throw") throw new Error("console unreachable");
      return { ok: p.ok, json: async () => p.body };
    }),
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("queryReattachTicket /reattach 契约", () => {
  it("ok：带 Bearer 头与编码 peer 查询，映射票据与到期时刻", async () => {
    const fetchMock = vi.fn(async (_url: string, _init?: RequestInit) => ({
      ok: true,
      json: async () => ({
        peer: "peer x",
        ticket: "tk-1",
        expires_at_unix_ms: 123456,
        reason: "ok",
      }),
    }));
    vi.stubGlobal("fetch", fetchMock);
    const answer = await queryReattachTicket("http://127.0.0.1:9900/", "tok", "peer x");
    expect(fetchMock.mock.calls[0][0]).toBe("http://127.0.0.1:9900/reattach?peer=peer%20x");
    expect((fetchMock.mock.calls[0][1] as { headers: Record<string, string> }).headers).toEqual({
      Authorization: "Bearer tok",
    });
    expect(answer).toEqual({
      ticket: "tk-1",
      expiresAtUnixMs: 123456,
      reason: "ok",
    });
  });

  it("missing/expired 如实映射；未知 reason 折化为 unavailable", async () => {
    stubFetch(() => ({ ok: true, body: { peer: "p", ticket: null, reason: "expired" } }));
    expect(await queryReattachTicket("http://s", "t", "p")).toMatchObject({
      ticket: null,
      reason: "expired",
    });
    stubFetch(() => ({ ok: true, body: { peer: "p", ticket: null, reason: "missing" } }));
    expect(await queryReattachTicket("http://s", "t", "p")).toMatchObject({ reason: "missing" });
    stubFetch(() => ({ ok: true, body: { peer: "p", weird: true } }));
    const unknown = await queryReattachTicket("http://s", "t", "p");
    expect(unknown).toEqual({ ticket: null, expiresAtUnixMs: null, reason: "unavailable" });
  });

  it("console 不可达/非 2xx：折化 unavailable 不抛出（fresh 拨号兜底）", async () => {
    stubFetch(() => "throw");
    const down = await queryReattachTicket("http://s", "t", "p");
    expect(down).toEqual({ ticket: null, expiresAtUnixMs: null, reason: "unavailable" });
    stubFetch(() => ({ ok: false, body: { error: "unauthorized" } }));
    const denied = await queryReattachTicket("http://s", "t", "p");
    expect(denied.reason).toBe("unavailable");
  });
});

describe("fetchDiscoveryPeers /discovery 契约", () => {
  it("映射 owner 名/来源/地址；非法条目与缺字段容错", async () => {
    stubFetch(() => ({
      ok: true,
      body: {
        peers: [
          { peer: "p1", addrs: ["/ip4/10.0.0.8/tcp/4001"], name: "home", source: "mdns" },
          { peer: "p2" },
          { peer: "p3", addrs: ["ok", 42, null], name: 7 },
          { peer: "" },
          42,
          null,
        ],
      },
    }));
    const peers = await fetchDiscoveryPeers("http://s", "t");
    expect(peers).toEqual([
      {
        peer: "p1",
        addrs: ["/ip4/10.0.0.8/tcp/4001"],
        name: "home",
        source: "mdns",
      },
      { peer: "p2", addrs: undefined, name: null, source: null },
      { peer: "p3", addrs: ["ok"], name: null, source: null },
    ]);
  });

  it("console 可达但无发现：空数组（非 null），空态引导不误报", async () => {
    stubFetch(() => ({ ok: true, body: { peers: [] } }));
    expect(await fetchDiscoveryPeers("http://s", "t")).toEqual([]);
  });

  it("console 不可达/坏形响应：null（调用方停止轮询）", async () => {
    stubFetch(() => "throw");
    expect(await fetchDiscoveryPeers("http://s", "t")).toBeNull();
    stubFetch(() => ({ ok: true, body: { nope: 1 } }));
    expect(await fetchDiscoveryPeers("http://s", "t")).toEqual([]);
    stubFetch(() => ({ ok: false, body: null }));
    expect(await fetchDiscoveryPeers("http://s", "t")).toBeNull();
  });
});
