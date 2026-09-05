import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WsLike } from "./ws-factory";

// §2.3 未读计数（agent）：连接期收到完整回复且未聚焦该端点时 unread+1；
// 聚焦（setFocusedEndpoint）清零且不再累积；最后一次交互时间随回合落地。
vi.stubEnv("VITE_MOCK_IPC", "1");

const { mockAcpConsole, MockSocket } = await import("./mock-acp-ws");
const { setWsFactory } = await import("./ws-factory");
const { useAcpStore } = await import("./acp-store");
await import("@/i18n");

const ENDPOINT_ID = "ep-unread-1";

async function connectEndpoint(): Promise<void> {
  useAcpStore.setState({
    draft: {
      wsUrl: "ws://127.0.0.1:8787",
      token: "mock-token",
      peer: "mock-peer",
      endpointId: ENDPOINT_ID,
      alias: "helper",
    },
    activeEndpointId: null,
    focusedEndpointId: null,
  });
  useAcpStore.getState().connect();
  await vi.waitFor(() => {
    expect(useAcpStore.getState().phase).toBe("online");
  });
  await useAcpStore.getState().newSession();
}

beforeEach(() => {
  localStorage.clear();
  mockAcpConsole.reset();
  useAcpStore.getState().resetConsoleState();
  setWsFactory((url: string) => new MockSocket(url, mockAcpConsole) as unknown as WsLike);
});

afterEach(() => {
  setWsFactory(null);
  useAcpStore.getState().disconnect();
});

describe("acp-store 端点未读与最后交互（§2.2/§2.3）", () => {
  it("未聚焦端点收到完整回复 unread+1 并记录最后交互时间", async () => {
    await connectEndpoint();
    expect(useAcpStore.getState().activeEndpointId).toBe(ENDPOINT_ID);
    const ok = await useAcpStore.getState().sendPrompt("你好");
    expect(ok).toBe(true);
    const state = useAcpStore.getState();
    expect(state.unreadByEndpoint[ENDPOINT_ID]).toBe(1);
    expect(state.lastInteractionByEndpoint[ENDPOINT_ID]).toBeGreaterThan(0);
  });

  it("聚焦端点（setFocusedEndpoint）清零且回复不再累积未读", async () => {
    await connectEndpoint();
    useAcpStore.getState().setFocusedEndpoint(ENDPOINT_ID);
    useAcpStore.setState({
      unreadByEndpoint: { [ENDPOINT_ID]: 3 },
    });
    useAcpStore.getState().setFocusedEndpoint(ENDPOINT_ID);
    expect(useAcpStore.getState().unreadByEndpoint[ENDPOINT_ID]).toBe(0);
    await useAcpStore.getState().sendPrompt("聚焦中");
    expect(useAcpStore.getState().unreadByEndpoint[ENDPOINT_ID]).toBe(0);
  });
});
