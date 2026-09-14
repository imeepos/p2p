// 会话侧栏渲染矩阵（uix-spec §2 两级树）：分组/折叠/当前组强制展开/行内限流/
// 搜索/选中态/空态/离线禁用/可访问性。AF1 追加：新建会话反馈闭环矩阵（跨入口
// pending 禁用/连点单发/成功与失败 toast 单弹）。
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { Toaster } from "sonner";
import { beforeEach, describe, expect, it } from "vitest";

import { useAcpStore } from "@/acp/acp-store";
import { AgentConversation } from "@/views/agent-chat/agent-conversation";
import type { SessionSummary } from "@/acp/protocol";
import { mockAcpConsole, MockSocket } from "@/acp/mock-acp-ws";
import { setWsFactory, type WsLike, type WebSocketFactory } from "@/acp/ws-factory";
import { MOCK_AGENT_INFO } from "@/acp/mock-script";
import { resetWorkspaceUiForTest } from "@/acp/workspace-ui-store";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { SessionSidebar } from "./session-sidebar";

await import("@/i18n");

// SessionRow 依赖全站确认弹框 context（关闭会话确认纪律），独立渲染需自持 Provider
function renderSidebar() {
  return render(
    <ConfirmProvider>
      <SessionSidebar />
    </ConfirmProvider>,
  );
}

function setSessions(sessions: SessionSummary[], activeSessionId: string | null = null) {
  useAcpStore.setState({ phase: "online", sessions, activeSessionId });
}

beforeEach(() => {
  useAcpStore.getState().resetConsoleState();
  resetWorkspaceUiForTest();
  setSessions([
    { sessionId: "s-1", title: "first" },
    { sessionId: "s-2", title: "second" },
  ], "s-2");
});

describe("SessionSidebar 两级树", () => {
  it("按 cwd 分组渲染组头，未分组组头恒排最后（I1）", () => {
    setSessions([
      { sessionId: "s-1", title: "a", cwd: "/tmp/alpha" },
      { sessionId: "s-2", title: "b" },
      { sessionId: "s-3", title: "c", cwd: "/tmp/beta" },
    ]);
    renderSidebar();
    expect(screen.getByTestId("acp-group-row-/tmp/alpha")).toBeTruthy();
    expect(screen.getByTestId("acp-group-row-/tmp/beta")).toBeTruthy();
    expect(screen.getByTestId("acp-group-row-ungrouped").textContent).toContain("未分组");
    const heads = screen.getAllByTestId(/^acp-group-row-/);
    expect(heads[heads.length - 1].dataset.testid).toBe("acp-group-row-ungrouped");
  });

  it("点组头折叠/展开：折叠后子行消失、aria-expanded=false（I3）", () => {
    setSessions([
      { sessionId: "s-1", title: "a", cwd: "/tmp/alpha" },
      { sessionId: "s-2", title: "b", cwd: "/tmp/beta" },
    ]);
    renderSidebar();
    expect(screen.getByTestId("acp-session-row-s-1")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-group-row-/tmp/alpha"));
    expect(screen.queryByTestId("acp-session-row-s-1")).toBeNull();
    expect(screen.getByTestId("acp-group-row-/tmp/alpha").getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(screen.getByTestId("acp-group-row-/tmp/alpha"));
    expect(screen.getByTestId("acp-session-row-s-1")).toBeTruthy();
  });

  it("含当前会话的组强制展开，折叠不生效（I3）", () => {
    setSessions([
      { sessionId: "s-1", title: "a", cwd: "/tmp/alpha" },
      { sessionId: "s-2", title: "b", cwd: "/tmp/beta" },
    ], "s-1");
    renderSidebar();
    fireEvent.click(screen.getByTestId("acp-group-row-/tmp/alpha"));
    expect(screen.getByTestId("acp-session-row-s-1")).toBeTruthy();
    // 对照组：不含当前会话的组可折叠
    fireEvent.click(screen.getByTestId("acp-group-row-/tmp/beta"));
    expect(screen.queryByTestId("acp-session-row-s-2")).toBeNull();
  });

  it("行内限流：7 条只显 5 行并显「展开 N 个」，点击临时全开（I4）", () => {
    setSessions(
      Array.from({ length: 7 }, (_, i) => ({ sessionId: "s-" + i, title: "t" + i, cwd: "/tmp/alpha" })),
    );
    renderSidebar();
    expect(screen.getByText("展开 2 个")).toBeTruthy();
    const visible = screen.getAllByTestId(/^acp-session-row-/);
    expect(visible).toHaveLength(5);
    fireEvent.click(screen.getByTestId("acp-group-expand-/tmp/alpha"));
    expect(screen.getAllByTestId(/^acp-session-row-/)).toHaveLength(7);
    expect(screen.queryByText("展开 2 个")).toBeNull();
  });

  it("搜索过滤会话，无结果显示空文案，Esc 清空还原树（I8）", () => {
    setSessions([
      { sessionId: "s-1", title: "alpha one", cwd: "/tmp/alpha" },
      { sessionId: "s-2", title: "beta two", cwd: "/tmp/beta" },
    ]);
    renderSidebar();
    const input = screen.getByTestId("acp-session-search") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "alpha" } });
    expect(screen.getByTestId("acp-session-row-s-1")).toBeTruthy();
    expect(screen.queryByTestId("acp-session-row-s-2")).toBeNull();
    fireEvent.change(input, { target: { value: "nomatch" } });
    expect(screen.getByText("无匹配会话")).toBeTruthy();
    fireEvent.keyDown(input, { key: "Escape" });
    expect(input.value).toBe("");
    expect(screen.getByTestId("acp-session-row-s-1")).toBeTruthy();
  });

  it("无会话显空态文案而非白板（I10）", () => {
    setSessions([]);
    renderSidebar();
    expect(screen.getByText("暂无会话")).toBeTruthy();
  });
});

describe("SessionSidebar 可访问性与离线禁用", () => {
  it("活动会话 aria-current=true，非活动会话无该属性", () => {
    renderSidebar();
    const active = screen.getByTestId("acp-session-row-s-2").querySelector("button");
    const inactive = screen.getByTestId("acp-session-row-s-1").querySelector("button");
    expect(active?.getAttribute("aria-current")).toBe("true");
    expect(inactive?.getAttribute("aria-current")).toBeNull();
  });

  it("关闭按钮为 icon-only 但带 aria-label", () => {
    renderSidebar();
    const close = screen.getByTestId("acp-session-close-s-1");
    expect(close.getAttribute("aria-label")).toBe("关闭会话");
  });

  it("离线时 resume/close/新建禁用，组头开合不受限", () => {
    useAcpStore.setState({ phase: "idle", activeSessionId: null });
    renderSidebar();
    const row = screen.getByTestId("acp-session-row-s-1");
    expect((row.querySelector("button") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId("acp-session-close-s-1") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId("acp-session-new") as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId("acp-group-row-ungrouped"));
    expect(screen.queryByTestId("acp-session-row-s-1")).toBeNull();
  });
});

const EP = {
  endpointId: "ep-1",
  wsUrl: "ws://127.0.0.1:8787",
  token: "mock-token",
  peer: "mock-peer",
};

function renderBothEntries() {
  return render(
    <MemoryRouter>
      <ConfirmProvider>
        <SessionSidebar />
        <AgentConversation endpointId="ep-1" />
        <Toaster position="top-right" />
      </ConfirmProvider>
    </MemoryRouter>,
  );
}

function seedOnlineView(): void {
  useAcpStore.setState({
    saved: [EP],
    activeEndpointId: "ep-1",
    activeSessionId: null,
  });
}

/** 走真实 mock 连接：connect 到 online，供点击触达 store → IPC 全链 */
async function connectMockAgent(): Promise<void> {
  setWsFactory((url) => new MockSocket(url, mockAcpConsole));
  useAcpStore.setState({ draft: { ...EP } });
  useAcpStore.getState().connect();
  await waitFor(() => {
    expect(useAcpStore.getState().phase).toBe("online");
  });
}

/** initialize 成功但 session/new 一律失败的 socket：失败链路确定性驱动 */
class FailNewSocket implements WsLike {
  onopen: (() => void) | null = null;
  onclose: ((ev: { code: number; reason: string }) => void) | null = null;
  onerror: ((ev: { message?: string }) => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  constructor() {
    window.setTimeout(() => this.onopen?.(), 0);
  }
  send(data: string): void {
    for (const line of data.split("\n")) {
      if (!line.trim()) continue;
      const msg = JSON.parse(line) as { id?: number; method?: string };
      if (typeof msg.id !== "number" || !msg.method) continue;
      if (msg.method === "initialize") {
        this.deliver({ jsonrpc: "2.0", id: msg.id, result: MOCK_AGENT_INFO });
      } else if (msg.method === "session/new") {
        this.deliver({
          jsonrpc: "2.0",
          id: msg.id,
          error: { code: -32000, message: "mock session/new rejected" },
        });
      } else {
        this.deliver({ jsonrpc: "2.0", id: msg.id, result: {} });
      }
    }
  }
  private deliver(msg: unknown): void {
    this.onmessage?.({ data: new TextEncoder().encode(JSON.stringify(msg) + "\n") });
  }
  close(): void {
    this.onclose?.({ code: 1000, reason: "client-close" });
  }
}

function connectFailNewAgent(): Promise<void> {
  const factory: WebSocketFactory = () => new FailNewSocket();
  setWsFactory(factory);
  useAcpStore.setState({ draft: { ...EP } });
  useAcpStore.getState().connect();
  return waitFor(() => {
    expect(useAcpStore.getState().phase).toBe("online");
  }).then(() => undefined);
}

describe("新建会话反馈闭环渲染矩阵（AF1）", () => {
  it("store pending 为 true 时两入口同时禁用，复位后恢复", () => {
    seedOnlineView();
    renderBothEntries();
    expect((screen.getByTestId("acp-session-new") as HTMLButtonElement).disabled).toBe(false);
    expect((screen.getByTestId("agent-new-session") as HTMLButtonElement).disabled).toBe(false);
    act(() => {
      useAcpStore.setState({ newSessionPending: true });
    });
    expect((screen.getByTestId("acp-session-new") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId("agent-new-session") as HTMLButtonElement).disabled).toBe(true);
    act(() => {
      useAcpStore.setState({ newSessionPending: false });
    });
    expect((screen.getByTestId("acp-session-new") as HTMLButtonElement).disabled).toBe(false);
    expect((screen.getByTestId("agent-new-session") as HTMLButtonElement).disabled).toBe(false);
  });

  it("连点只触发一次 session/new，成功 toast 单弹「会话已创建」", async () => {
    seedOnlineView();
    renderBothEntries();
    await connectMockAgent();
    fireEvent.click(screen.getByTestId("agent-new-session"));
    fireEvent.click(screen.getByTestId("agent-new-session"));
    expect(useAcpStore.getState().newSessionPending).toBe(true);
    expect((screen.getByTestId("agent-new-session") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId("acp-session-new") as HTMLButtonElement).disabled).toBe(true);
    const created = await screen.findByText("会话已创建");
    expect(created).toBeTruthy();
    expect(screen.queryAllByText("会话已创建")).toHaveLength(1);
    await waitFor(() => {
      expect(useAcpStore.getState().activeSessionId).toBe("s-001");
    });
    expect(useAcpStore.getState().newSessionPending).toBe(false);
  });

  it("失败时错误 toast 只出现一次（不与新链路双弹），结束后按钮恢复", async () => {
    seedOnlineView();
    renderBothEntries();
    await connectFailNewAgent();
    fireEvent.click(screen.getByTestId("acp-session-new"));
    const failure = await screen.findByText("新建会话失败");
    expect(failure).toBeTruthy();
    expect(screen.queryAllByText("新建会话失败")).toHaveLength(1);
    expect(useAcpStore.getState().lastError).toBe("sessionNewFailed");
    expect(useAcpStore.getState().newSessionPending).toBe(false);
    await waitFor(
      () => {
        expect((screen.getByTestId("acp-session-new") as HTMLButtonElement).disabled).toBe(false);
        expect((screen.getByTestId("agent-new-session") as HTMLButtonElement).disabled).toBe(false);
      },
      { timeout: 4_000 },
    );
  });
});
