import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// 契约 §15 console-watch 自动流程覆盖（in-process pump 三态）：
// 1) connected -> 本机 agent 端点自动登记（稳定 id + 默认别名）且相位重放幂等（不重复登记、不重复连接）；
// 2) 登记后自动连接 + 自动开新会话（发现面解析 peer）；
// 3) disconnected 相位显式落 store（不静默），恢复 connected 后流程可重入。
vi.stubEnv("VITE_MOCK_IPC", "1");

const { mockAcpConsole: mockAcpWs, MockSocket } = await import("./mock-acp-ws");
const { mockAcpConsole } = await import("@/lib/mock-acp-console");
const { setWsFactory } = await import("./ws-factory");
const { useAcpStore } = await import("./acp-store");
const { ensureConsoleWatch, resetConsoleWatchForTest } = await import("./console-watch");
const { LOCAL_AGENT_ENDPOINT_ID } = await import("./console-client");
await import("@/i18n");

const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";

function stubDiscoveryFetch(): void {
  vi.stubGlobal("fetch", vi.fn(async () => ({
    ok: true,
    json: async () => ({ peers: [{ peer: PEER, addrs: [], source: "mdns" }] }),
  })));
}

// token 与 mock-acp-console 快照默认值一致：快照先于事件到达时登记值不漂移
async function emitConnected(): Promise<void> {
  mockAcpConsole.emit({
    phase: "connected",
    wsUrl: "ws://127.0.0.1:8787",
    token: "mock-console-token",
    statusUrl: "http://127.0.0.1:8788",
  });
}

beforeEach(() => {
  localStorage.clear();
  mockAcpConsole.reset();
  mockAcpWs.reset();
  // WS mock 对齐 status 快照的连接面（token 同 mock-acp-console 快照默认值）
  mockAcpWs.configure({ token: "mock-console-token", peers: [PEER] });
  resetConsoleWatchForTest();
  useAcpStore.getState().resetConsoleState();
  // saved/draft 为持久化档（resetConsoleState 不清）：显式复位保用例隔离
  useAcpStore.setState({ saved: [], draft: { wsUrl: "ws://127.0.0.1:8787", token: "", peer: "" } });
  setWsFactory((url: string) => new MockSocket(url, mockAcpWs) as never);
});

afterEach(() => {
  resetConsoleWatchForTest();
  setWsFactory(null);
  vi.unstubAllGlobals();
  useAcpStore.getState().disconnect();
});

describe("console-watch 自动登记与零点击直达（契约 §15）", () => {
  it("connected 即自动登记本机 agent（稳定 id/默认别名/console 连接面），发现面解析 peer 后自动连接并自动开会话", async () => {
    stubDiscoveryFetch();
    ensureConsoleWatch();
    emitConnected();
    const state = useAcpStore.getState();
    const local = state.saved.find((e) => e.endpointId === LOCAL_AGENT_ENDPOINT_ID);
    expect(local).toBeTruthy();
    expect(local!.alias).toBe("本机 agent");
    expect(local!.wsUrl).toBe("ws://127.0.0.1:8787");
    expect(local!.token).toBe("mock-console-token");
    expect(local!.statusUrl).toBe("http://127.0.0.1:8788");
    // 自动连接 + 自动开会话：/chat?agent=<本机id> 直落会话的前提
    await vi.waitFor(() => {
      expect(useAcpStore.getState().phase).toBe("online");
      expect(useAcpStore.getState().activeEndpointId).toBe(LOCAL_AGENT_ENDPOINT_ID);
    });
    await vi.waitFor(() => {
      expect(useAcpStore.getState().activeSessionId).not.toBeNull();
    });
    const saved = useAcpStore.getState().saved;
    expect(saved.filter((e) => e.endpointId === LOCAL_AGENT_ENDPOINT_ID)).toHaveLength(1);
    expect(saved.find((e) => e.endpointId === LOCAL_AGENT_ENDPOINT_ID)!.peer).toBe(PEER);
  });

  it("相位重放幂等：同 connected 状态重发不产生重复端点、不发起重复连接（无风暴）", async () => {
    stubDiscoveryFetch();
    ensureConsoleWatch();
    emitConnected();
    await vi.waitFor(() => expect(useAcpStore.getState().activeSessionId).not.toBeNull());
    const dialsBefore = useAcpStore.getState().phase;
    // 重放同相位 + 重发事件：登记数不变、连接不重拨（会话不变）
    ensureConsoleWatch();
    emitConnected();
    emitConnected();
    await new Promise((r) => setTimeout(r, 20));
    const saved = useAcpStore.getState().saved;
    expect(saved.filter((e) => e.endpointId === LOCAL_AGENT_ENDPOINT_ID)).toHaveLength(1);
    expect(useAcpStore.getState().phase).toBe(dialsBefore);
    expect(useAcpStore.getState().activeSessionId).not.toBeNull();
  });

  it("disconnected 相位显式落快照；恢复 connected 后登记连接流程可重入", async () => {
    stubDiscoveryFetch();
    ensureConsoleWatch();
    mockAcpConsole.emit({ phase: "disconnected", lastError: "pump task exited" });
    expect(useAcpStore.getState().console).toMatchObject({ phase: "disconnected" });
    expect(useAcpStore.getState().saved).toHaveLength(0);
    // 泵恢复（新连接面）：重新登记且完成自动连接
    emitConnected();
    await vi.waitFor(() => {
      expect(useAcpStore.getState().activeEndpointId).toBe(LOCAL_AGENT_ENDPOINT_ID);
      expect(useAcpStore.getState().phase).toBe("online");
    });
  });
});
