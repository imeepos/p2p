import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

// R2-24 验收：chat agent 形态待应答权限的可见性与深链；无待应答零痕迹。
// mock 回放脚本无法自然触达权限请求，登记走 store 注入（与生产消费
// request_permission 帧后的 store 形状一致），并顺带断言 window 注入路径。
// 全动态导入：stubEnv 先于模块求值（console-watch 绑定 tauri 后端）。
vi.stubEnv("VITE_MOCK_IPC", "1");
vi.resetModules();

const { useAcpStore } = await import("@/acp/acp-store");
const { AgentConversation } = await import("./agent-conversation");
await import("@/i18n");

function seedConnected(): string {
  useAcpStore.setState({
    saved: [
      {
        endpointId: "ep-1",
        wsUrl: "ws://127.0.0.1:8787",
        token: "mock-token",
        peer: "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX",
        alias: "编码助手",
      },
    ],
    console: {
      phase: "ready",
      wsUrl: "ws://127.0.0.1:8787",
      token: "mock-token",
      statusUrl: "http://127.0.0.1:8788",
      adminUrl: "http://127.0.0.1:8790",
      restarts: 0,
    },
    phase: "online",
    activeEndpointId: "ep-1",
    activeSessionId: "s-1",
  });
  return "ep-1";
}

function interactionWith(status: "pending" | "approved") {
  return {
    permissions: [
      {
        requestId: 7,
        sessionId: "s-1",
        title: "执行命令",
        toolKind: "execute",
        options: [],
        receivedAt: Date.now(),
        status,
      },
    ],
    configOptions: [],
    usage: null,
  };
}

function renderAgent(endpointId: string): void {
  render(
    <MemoryRouter>
      <AgentConversation endpointId={endpointId} />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  localStorage.clear();
  useAcpStore.getState().resetConsoleState();
});

describe("chat agent 形态待应答权限指示（R2-24）", () => {
  it("无待应答时零痕迹：不渲染任何指示节点", () => {
    const ep = seedConnected();
    useAcpStore.setState({ interactions: {} });
    renderAgent(ep);
    expect(screen.getByTestId("agent-conversation")).toBeTruthy();
    expect(screen.queryByTestId("agent-permission-banner")).toBeNull();
  });

  it("存在待应答权限：指示条出计数文案，深链直达通讯录权限面板", () => {
    const ep = seedConnected();
    useAcpStore.setState({ interactions: { "s-1": interactionWith("pending") } });
    renderAgent(ep);
    expect(screen.getByTestId("agent-permission-banner")).toBeTruthy();
    expect(screen.getByTestId("agent-permission-banner-text").textContent).toContain("1");
    const action = screen.getByTestId("agent-permission-banner-action");
    expect(action.tagName).toBe("A");
    expect(action.getAttribute("href")).toBe("/contacts?agentDetail=ep-1");
  });

  it("权限已决（approved）不算待应答：指示条不出现", () => {
    const ep = seedConnected();
    useAcpStore.setState({ interactions: { "s-1": interactionWith("approved") } });
    renderAgent(ep);
    expect(screen.queryByTestId("agent-permission-banner")).toBeNull();
  });

  it("mock 态 window 注入路径可用：调用即登记待应答并点亮指示条", () => {
    const ep = seedConnected();
    useAcpStore.setState({ interactions: {} });
    const inject = (window as unknown as {
      __acpInjectPendingPermission?: (sessionId?: string) => void;
    }).__acpInjectPendingPermission;
    expect(typeof inject).toBe("function");
    inject!();
    renderAgent(ep);
    expect(screen.getByTestId("agent-permission-banner")).toBeTruthy();
    const pending = useAcpStore.getState().interactions["s-1"]!.permissions;
    expect(pending).toHaveLength(1);
    expect(pending[0]!.status).toBe("pending");
  });
});