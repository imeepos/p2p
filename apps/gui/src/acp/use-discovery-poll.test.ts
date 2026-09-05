// 发现轮询 hook：挂载即拉、周期拉、退后台暂停/恢复、不可达停轮询（不误报）。
import { renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { DISCOVERY_POLL_INTERVAL_MS, useDiscoveryPoll } from "./use-discovery-poll";
import { useAcpStore } from "./acp-store";

const fetchMock = vi.fn();
function stubPeers(payload: () => unknown): void {
  fetchMock.mockImplementation(async () => {
    const body = payload();
    if (body === "throw") throw new Error("down");
    return { ok: true, json: async () => body };
  });
  vi.stubGlobal("fetch", fetchMock);
}

beforeEach(() => {
  fetchMock.mockReset();
  useAcpStore.getState().resetConsoleState();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("useDiscoveryPoll", () => {
  it("挂载即拉并进目录，周期续拉（目标二）", async () => {
    vi.useFakeTimers();
    stubPeers(() => ({ peers: [{ peer: "p-1", addrs: ["/ip4/1/tcp/1"], source: "mdns" }] }));
    const { unmount } = renderHook(() =>
      useDiscoveryPoll({ statusUrl: "http://127.0.0.1:1", token: "t" }),
    );
    await vi.advanceTimersByTimeAsync(0);
    expect(useAcpStore.getState().directory.map((e) => e.peer)).toEqual(["p-1"]);
    await vi.advanceTimersByTimeAsync(DISCOVERY_POLL_INTERVAL_MS);
    expect(fetchMock.mock.calls.length).toBe(2);
    unmount();
    await vi.advanceTimersByTimeAsync(DISCOVERY_POLL_INTERVAL_MS * 2);
    expect(fetchMock.mock.calls.length).toBe(2);
  });

  it("console 不可达：首轮即停，不再轮询且无错误上墙（目标二）", async () => {
    vi.useFakeTimers();
    stubPeers(() => "throw");
    const { unmount } = renderHook(() =>
      useDiscoveryPoll({ statusUrl: "http://127.0.0.1:1", token: "t" }),
    );
    await vi.advanceTimersByTimeAsync(0);
    expect(fetchMock.mock.calls.length).toBe(1);
    await vi.advanceTimersByTimeAsync(DISCOVERY_POLL_INTERVAL_MS * 3);
    expect(fetchMock.mock.calls.length).toBe(1);
    unmount();
  });

  it("缺 statusUrl 或退后台：不轮询（目标二）", async () => {
    vi.useFakeTimers();
    stubPeers(() => ({ peers: [] }));
    const noUrl = renderHook(() => useDiscoveryPoll({ statusUrl: null, token: "t" }));
    await vi.advanceTimersByTimeAsync(DISCOVERY_POLL_INTERVAL_MS);
    expect(fetchMock.mock.calls.length).toBe(0);
    noUrl.unmount();

    const hidden = vi.spyOn(document, "hidden", "get").mockReturnValue(true);
    const mounted = renderHook(() =>
      useDiscoveryPoll({ statusUrl: "http://127.0.0.1:1", token: "t" }),
    );
    await vi.advanceTimersByTimeAsync(DISCOVERY_POLL_INTERVAL_MS * 2);
    expect(fetchMock.mock.calls.length).toBe(0);
    mounted.unmount();
    hidden.mockRestore();
  });
});
