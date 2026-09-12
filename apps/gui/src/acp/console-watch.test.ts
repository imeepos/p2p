import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// 契约 §15 console-watch 自动流程覆盖（in-process pump 三态）：
// 1) connected -> 本机 agent 端点自动登记（稳定 id + 默认别名）且相位重放幂等（不重复登记、不重复连接）；
// 2) 登记后自动连接 + 自动开新会话（发现面解析 peer）；
// 3) disconnected 相位显式落 store（不静默），恢复 connected 后流程可重入。
vi.stubEnv("VITE_MOCK_IPC", "1");

const { mockAcpConsole: mockAcpWs, MockSocket } = await import("./mock-acp-ws");
const { mockAcpConsole } = await import("@/lib/mock-acp-console");
const { mockBackend } = await import("@/lib/mock-ipc");
const { setWsFactory } = await import("./ws-factory");
const { useAcpStore } = await import("./acp-store");
const { ensureConsoleWatch, resetConsoleWatchForTest } = await import("./console-watch");
const { LOCAL_AGENT_ENDPOINT_ID } = await import("./console-client");
await import("@/i18n");

const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";
const DESC_PEER = "2mzDescStubPeer9XGGUa3KguQQGc1Mc8LwNsVUZXk";

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
  vi.restoreAllMocks();
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

describe("console-watch 零配置 peer 解序（描述文件优先，发现面回落）", () => {
  it("本机自描述带 peer：开箱即连，全程不触碰发现面", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    const spy = vi.spyOn(mockBackend, "acpLocalDescriptor").mockResolvedValue({
      adminUrl: "http://127.0.0.1:8123",
      token: "desc-tok",
      peer: DESC_PEER,
      agentName: "home-agent",
      writtenAtUnix: 1_725_700_000,
    });
    try {
      // mock WS 校验 peer 白名单：descriptor 直取的 peer 必须在拨号白名单内
      mockAcpWs.configure({ token: "mock-console-token", peers: [DESC_PEER] });
      ensureConsoleWatch();
      emitConnected();
      await vi.waitFor(() => {
        expect(useAcpStore.getState().phase).toBe("online");
        expect(useAcpStore.getState().activeEndpointId).toBe(LOCAL_AGENT_ENDPOINT_ID);
      });
      const local = useAcpStore
        .getState()
        .saved.find((e) => e.endpointId === LOCAL_AGENT_ENDPOINT_ID);
      expect(local!.peer).toBe(DESC_PEER);
      expect(fetchMock).not.toHaveBeenCalled();
    } finally {
      spy.mockRestore();
    }
  });

  it("存量端点已带 peer：直接连接，不再查询描述文件（幂等零开销）", async () => {
    useAcpStore.setState({
      saved: [
        {
          endpointId: LOCAL_AGENT_ENDPOINT_ID,
          wsUrl: "ws://127.0.0.1:8787",
          token: "mock-console-token",
          peer: PEER,
          alias: "本机 agent",
        },
      ],
    });
    const spy = vi.spyOn(mockBackend, "acpLocalDescriptor");
    try {
      ensureConsoleWatch();
      emitConnected();
      await vi.waitFor(() => expect(useAcpStore.getState().phase).toBe("online"));
      expect(spy).not.toHaveBeenCalled();
    } finally {
      spy.mockRestore();
    }
  });

  it("描述缺失（agent 从未落盘）：回落发现面轮询解析 peer", async () => {
    stubDiscoveryFetch();
    const spy = vi.spyOn(mockBackend, "acpLocalDescriptor").mockResolvedValue(null);
    try {
      ensureConsoleWatch();
      emitConnected();
      await vi.waitFor(() => {
        expect(useAcpStore.getState().phase).toBe("online");
      });
      expect(
        useAcpStore
          .getState()
          .saved.find((e) => e.endpointId === LOCAL_AGENT_ENDPOINT_ID)!.peer,
      ).toBe(PEER);
    } finally {
      spy.mockRestore();
    }
  });
});
