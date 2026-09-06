import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

// UX3 验收：console failed/unavailable 相位显式引导卡（安装/日志指引，不静默）；
// ready 且登记后自动连接+自动开会话，/chat?agent=<本机id> 直落会话零二次点击。
// 全动态导入：stubEnv 必须先于模块求值（静态 import 提升会让 console-watch
// 在 env 就位前绑定 tauri 后端），resetModules 保证整链按 mock 态重求值。
vi.stubEnv("VITE_MOCK_IPC", "1");
vi.resetModules();

const { mockAcpConsole: mockAcpWs, MockSocket } = await import("@/acp/mock-acp-ws");
const { mockAcpConsole } = await import("@/lib/mock-acp-console");
const { setWsFactory } = await import("@/acp/ws-factory");
const { useAcpStore } = await import("@/acp/acp-store");
const { ensureConsoleWatch, resetConsoleWatchForTest } = await import("@/acp/console-watch");
const { LOCAL_AGENT_ENDPOINT_ID } = await import("@/acp/console-client");
const { AgentConversation } = await import("./agent-conversation");
await import("@/i18n");


const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";

function readyStatus() {
  return {
    phase: "ready" as const,
    wsUrl: "ws://127.0.0.1:8787",
    token: "mock-console-token",
    statusUrl: "http://127.0.0.1:8788",
    adminUrl: "http://127.0.0.1:8790",
    restarts: 0,
  };
}

function renderConversation(): void {
  render(
    <MemoryRouter>
      <AgentConversation endpointId={LOCAL_AGENT_ENDPOINT_ID} />
    </MemoryRouter>,
  );
}

function seedLocalEndpoint(): void {
  useAcpStore.setState({
    saved: [
      {
        endpointId: LOCAL_AGENT_ENDPOINT_ID,
        wsUrl: "ws://127.0.0.1:8787",
        token: "mock-console-token",
        peer: PEER,
        alias: "本机 agent",
        statusUrl: "http://127.0.0.1:8788",
      },
    ],
  });
}

beforeEach(() => {
  localStorage.clear();
  mockAcpConsole.reset();
  mockAcpWs.reset();
  mockAcpWs.configure({ token: "mock-console-token", peers: [PEER] });
  resetConsoleWatchForTest();
  useAcpStore.getState().resetConsoleState();
  useAcpStore.setState({ saved: [], draft: { wsUrl: "ws://127.0.0.1:8787", token: "", peer: "" } });
  setWsFactory((url: string) => new MockSocket(url, mockAcpWs) as never);
});

describe("agent 会话 console 引导卡与直落（UX3）", () => {
  it("failed 相位显引导卡：失败指引 + lastError + 查看日志入口，无手动连接按钮", () => {
    seedLocalEndpoint();
    useAcpStore.setState({ console: { phase: "failed", restarts: 5, lastError: "exit status 1" } });
    renderConversation();
    expect(screen.getByTestId("agent-console-guide")).toBeTruthy();
    expect(screen.getByTestId("agent-console-guide-hint").textContent).toBe(
      "acp-console 连续自动重启失败：请查看日志排查",
    );
    expect(screen.getByTestId("agent-console-guide-error").textContent).toContain("exit status 1");
    expect(screen.getByTestId("agent-console-guide-logs").getAttribute("href")).toBe("/diagnostics");
    expect(screen.queryByTestId("agent-connect")).toBeNull();
  });

  it("unavailable 相位显引导卡：安装指引；端点未登记时同样引导而非裸 notFound", () => {
    useAcpStore.setState({ console: { phase: "unavailable", restarts: 0 } });
    renderConversation();
    expect(screen.getByTestId("agent-console-guide")).toBeTruthy();
    expect(screen.getByTestId("agent-console-guide-hint").textContent).toContain("ACP_CONSOLE_BIN");
    expect(screen.queryByTestId("agent-connect")).toBeNull();
  });

  it("restarting 相位显重启进行时提示（非故障指引，无日志入口）", () => {
    seedLocalEndpoint();
    useAcpStore.setState({ console: { phase: "restarting", restarts: 2 } });
    renderConversation();
    expect(screen.getByTestId("agent-console-guide")).toBeTruthy();
    expect(screen.getByTestId("agent-console-guide-hint").textContent).toContain("自动重启");
    expect(screen.queryByTestId("agent-console-guide-logs")).toBeNull();
  });

  it("console ready 时本机 agent 不显引导卡：正常连接卡", () => {
    seedLocalEndpoint();
    useAcpStore.setState({ console: readyStatus() });
    renderConversation();
    expect(screen.getByTestId("agent-connect-card")).toBeTruthy();
    expect(screen.queryByTestId("agent-console-guide")).toBeNull();
  });

  it("连接失败行内出人话与可复制详情，错误码不再直显（F06）", () => {
    seedLocalEndpoint();
    useAcpStore.setState({ console: readyStatus() });
    useAcpStore.setState({ lastError: "endpointIncomplete" });
    renderConversation();
    const text = screen.getByTestId("agent-connect-error-text").textContent ?? "";
    expect(text).toContain("缺少 Token");
    expect(text).toContain("高级设置");
    expect(text).not.toContain("[endpointIncomplete]");
    const copy = screen.getByTestId("agent-connect-error-copy");
    expect(copy.getAttribute("title")).toContain("error=endpointIncomplete");
    expect(copy.getAttribute("title")).toContain("ws://127.0.0.1:8787");
  });

  it("自动流程就绪后 /chat?agent=<本机id> 直落会话：transcript 就位零二次点击", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({ ok: true, json: async () => ({ peers: [{ peer: PEER, addrs: [], source: "mdns" }] }) })),
    );
    ensureConsoleWatch();
    mockAcpConsole.emit(readyStatus());
    await vi.waitFor(() => {
      expect(useAcpStore.getState().phase).toBe("online");
      expect(useAcpStore.getState().activeSessionId).not.toBeNull();
    });
    renderConversation();
    await waitFor(() => expect(screen.getByTestId("agent-conversation")).toBeTruthy());
    // transcript 挂载（新会话空态文案）＝直落会话而非连接引导卡
    expect(screen.getByText("暂无消息")).toBeTruthy();
  });
});