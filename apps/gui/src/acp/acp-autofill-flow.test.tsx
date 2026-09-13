// 首用零手填流程回归：空白 token/peer 点连接自动补全后直连在线；补不齐保持 endpointIncomplete。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AcpLocalDescriptor } from "@/lib/ipc-types";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { AcpView } = await import("./acp-view");
const { mockAcpConsole: mockAcpWs } = await import("./mock-acp-ws");
const { mockBackend } = await import("@/lib/mock-ipc");
const { useAcpStore } = await import("./acp-store");
await import("@/i18n");

const DESCRIPTOR: AcpLocalDescriptor = {
  adminUrl: "http://127.0.0.1:8123",
  token: "desc-tok",
  peer: "mock-peer",
  agentName: "home-agent",
  writtenAtUnix: 1_725_700_000,
};

beforeEach(() => {
  localStorage.clear();
  // 补全链两端对齐：console 快照发的 token 必须在 mock WS 白名单内（真机同理由 pump 生成分发）
  mockAcpWs.reset();
  mockAcpWs.configure({ token: "mock-console-token", peers: ["mock-peer"] });
  useAcpStore.getState().resetConsoleState();
  useAcpStore.setState({
    phase: "idle",
    reconnect: null,
    draft: { wsUrl: "ws://127.0.0.1:8787", token: "", peer: "" },
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  useAcpStore.getState().disconnect();
});

describe("connect 前自动补全", () => {
  it("空白 token/peer 点连接：console 快照 + 自描述补齐后直连在线，草稿回填可见", async () => {
    vi.spyOn(mockBackend, "acpConsoleStatus").mockResolvedValue({
      phase: "connected",
      wsUrl: "ws://127.0.0.1:8787",
      token: "mock-console-token",
      statusUrl: "http://127.0.0.1:8788",
    });
    vi.spyOn(mockBackend, "acpLocalDescriptor").mockResolvedValue(DESCRIPTOR);
    render(<AcpView />);
    fireEvent.click(screen.getByTestId("acp-connect"));
    await waitFor(
      () => {
        expect(useAcpStore.getState().phase).toBe("online");
      },
      { timeout: 10_000 },
    );
    const draft = useAcpStore.getState().draft;
    expect(draft.token).toBe("mock-console-token");
    expect(draft.peer).toBe("mock-peer");
    expect(useAcpStore.getState().lastError).toBeNull();
  });

  it("补不齐（console 不可达 + 无自描述）：告警留痕，维持 endpointIncomplete 不误连", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    vi.spyOn(mockBackend, "acpConsoleStatus").mockRejectedValue(new Error("pump down"));
    vi.spyOn(mockBackend, "acpLocalDescriptor").mockResolvedValue(null);
    render(<AcpView />);
    fireEvent.click(screen.getByTestId("acp-connect"));
    await waitFor(() => {
      expect(useAcpStore.getState().lastError).toBe("endpointIncomplete");
    });
    expect(useAcpStore.getState().phase).toBe("idle");
    expect(warnSpy.mock.calls.some((c) => String(c[0]).includes("自动补全"))).toBe(true);
  });
});
