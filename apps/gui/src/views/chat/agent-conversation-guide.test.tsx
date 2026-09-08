import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

// 契约 §15 验收（in-process pump 三态）：console disconnected 相位显引导卡
// （失败留痕 + 日志指引，不静默）、connecting 相位显进行时；connected 且登记后
// 自动连接+自动开会话，/chat?agent=<本机id> 直落会话零二次点击。
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

function connectedStatus() {
  return {
    phase: "connected" as const,
    wsUrl: "ws://127.0.0.1:8787",
    token: "mock-console-token",
    statusUrl: "http://127.0.0.1:8788",
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

describe("agent 会话 console 引导卡与直落（契约 §15）", () => {
  it("disconnected 相位显引导卡：失败留痕 + lastError + 查看日志入口，无手动连接按钮", () => {
    seedLocalEndpoint();
    useAcpStore.setState({ console: { phase: "disconnected", lastError: "panic: boom" } });
    renderConversation();
    expect(screen.getByTestId("agent-console-guide")).toBeTruthy();
    expect(screen.getByTestId("agent-console-guide-hint").textContent).toBe(
      "本机 agent 泵已断开：请查看日志排查",
    );
    expect(screen.getByTestId("agent-console-guide-error").textContent).toContain("panic: boom");
    expect(screen.getByTestId("agent-console-guide-logs").getAttribute("href")).toBe("/diagnostics");
    expect(screen.queryByTestId("agent-connect")).toBeNull();
  });

  it("disconnected 相位端点未登记时同样引导而非裸 notFound", () => {
    useAcpStore.setState({ console: { phase: "disconnected" } });
    renderConversation();
    expect(screen.getByTestId("agent-console-guide")).toBeTruthy();
    expect(screen.getByTestId("agent-console-guide-hint").textContent).toContain("泵已断开");
    expect(screen.queryByTestId("agent-connect")).toBeNull();
  });

  it("connecting 相位显进行时提示（非故障指引，无日志入口）", () => {
    seedLocalEndpoint();
    useAcpStore.setState({ console: { phase: "connecting" } });
    renderConversation();
    expect(screen.getByTestId("agent-console-guide")).toBeTruthy();
    expect(screen.getByTestId("agent-console-guide-hint").textContent).toContain("装配");
    expect(screen.queryByTestId("agent-console-guide-logs")).toBeNull();
  });

  it("console connected 时本机 agent 不显引导卡：正常连接卡", () => {
    seedLocalEndpoint();
    useAcpStore.setState({ console: connectedStatus() });
    renderConversation();
    expect(screen.getByTestId("agent-connect-card")).toBeTruthy();
    expect(screen.queryByTestId("agent-console-guide")).toBeNull();
  });

  it("连接失败行内出人话与可复制详情，错误码不再直显（F06）", () => {
    seedLocalEndpoint();
    useAcpStore.setState({ console: connectedStatus() });
    useAcpStore.setState({ lastError: "endpointIncomplete" });
    renderConversation();
    const text = screen.getByTestId("agent-connect-error-text").textContent ?? "";
    expect(text).toContain("缺少 Token");
    expect(text).toContain("通讯录");
    expect(text).not.toContain("高级设置");
    expect(text).not.toContain("连接卡");
    expect(text).not.toContain("[endpointIncomplete]");
    const copy = screen.getByTestId("agent-connect-error-copy");
    expect(copy.getAttribute("title")).toContain("error=endpointIncomplete");
    expect(copy.getAttribute("title")).toContain("ws://127.0.0.1:8787");
  });

  it("R2-23 连接失败卡出编辑直达：指向 contacts?agentDetail 深链", () => {
    seedLocalEndpoint();
    useAcpStore.setState({ console: connectedStatus() });
    useAcpStore.setState({ lastError: "endpointIncomplete" });
    renderConversation();
    // asChild：testid 即渲染出的 <a> 本体
    const editLink = screen.getByTestId("agent-edit-link");
    expect(editLink.tagName).toBe("A");
    expect(editLink.getAttribute("href")).toBe(
      "/contacts?agentDetail=" + encodeURIComponent(LOCAL_AGENT_ENDPOINT_ID),
    );
  });

  it("自动流程就绪后 /chat?agent=<本机id> 直落会话：transcript 就位零二次点击", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({ ok: true, json: async () => ({ peers: [{ peer: PEER, addrs: [], source: "mdns" }] }) })),
    );
    ensureConsoleWatch();
    mockAcpConsole.emit(connectedStatus());
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
