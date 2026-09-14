import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAcpStore } from "@/acp/acp-store";
import { LOCAL_AGENT_ENDPOINT_ID } from "@/acp/console-client";
import type { AcpEndpoint } from "@/acp/protocol";
import { formatRelative } from "@/lib/relative-time";
import { installMatchMedia, resetViewport, setViewportNarrow } from "@/test/chat-page-fixtures";
import { AgentChatPage } from "./agent-chat-page";
import "@/i18n";

// ACS1 渲染矩阵：空态 / disconnected / connecting / connected+会话选中态 /
// 本机 console 断开引导 / 端点空清单 / <768 单栏互斥。矩阵只驱动 store 既有
// 切片（禁新增 IPC），不发真实连接。
const REMOTE = "ep-remote-1";
const LOCAL_ID = LOCAL_AGENT_ENDPOINT_ID;
const SESSION_A = "s-001";
const SESSION_B = "s-002";

function remoteEndpoint(): AcpEndpoint {
  return {
    endpointId: REMOTE,
    wsUrl: "ws://127.0.0.1:8787",
    token: "t",
    peer: "remote-peer",
    alias: "远端助手",
  };
}

function remoteEndpointB(): AcpEndpoint {
  return {
    endpointId: "ep-remote-2",
    wsUrl: "ws://10.0.0.9:8787",
    token: "t2",
    peer: "remote-peer-2",
    alias: "远端助手乙",
  };
}

function localEndpoint(): AcpEndpoint {  return {
    endpointId: LOCAL_ID,
    wsUrl: "ws://127.0.0.1:8787",
    token: "mock-token",
    peer: "local-peer",
    alias: "本机 agent",
  };
}

interface Seed {
  saved?: AcpEndpoint[];
  phase?: "idle" | "connecting" | "online" | "reconnecting" | "offline";
  activeEndpointId?: string | null;
  activeSessionId?: string | null;
  sessions?: { sessionId: string; title?: string; cwd?: string }[];
  unreadByEndpoint?: Record<string, number>;
  lastInteractionByEndpoint?: Record<string, number>;
  console?: { phase: "connecting" | "connected" | "disconnected"; lastError?: string } | null;
}

function seedStore(seed: Seed): void {
  useAcpStore.setState({
    saved: seed.saved ?? [remoteEndpoint()],
    phase: seed.phase ?? "idle",
    activePeer: null,
    activeEndpointId: seed.activeEndpointId ?? null,
    activeSessionId: seed.activeSessionId ?? null,
    sessions: seed.sessions ?? [],
    unreadByEndpoint: seed.unreadByEndpoint ?? {},
    lastInteractionByEndpoint: seed.lastInteractionByEndpoint ?? {},
    console: seed.console ?? null,
    focusedEndpointId: null,
    newSessionPending: false,
    transcripts: {},
    promptDrafts: {},
    promptPendingBySession: {},
  });
}

function renderPage(entry = "/agent"): void {
  render(
    <MemoryRouter initialEntries={[entry]}>
      <AgentChatPage />
    </MemoryRouter>,
  );
}

let errorSpy: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  localStorage.clear();
  installMatchMedia();
  resetViewport();
  useAcpStore.getState().resetConsoleState();
  errorSpy = vi.spyOn(console, "error");
});

afterEach(() => {
  // ACS1 验收第 6 条：矩阵全程 console 零新增报错
  expect(errorSpy).not.toHaveBeenCalled();
  errorSpy.mockRestore();
  cleanup();
  resetViewport();
});

describe("AgentChatPage 渲染矩阵", () => {
  it("空态：三区骨架在，未选端点时中间显选择引导", () => {
    seedStore({});
    renderPage();
    expect(screen.getByTestId("agent-chat-page")).toBeTruthy();
    expect(screen.getByTestId("agent-endpoint-sidebar")).toBeTruthy();
    expect(screen.getByTestId("agent-chat-conversation-pane")).toBeTruthy();
    expect(screen.getByText("选择端点开始会话")).toBeTruthy();
    expect(screen.queryByTestId("agent-conversation")).toBeNull();
  });

  it("端点空清单：既无 saved 也无 console 快照时显空态", () => {
    seedStore({ saved: [] });
    renderPage();
    expect(screen.getByText("暂无端点，先到通讯录添加智能体")).toBeTruthy();
  });

  it("disconnected：选中未连接端点显连接卡 + 离线会话提示", () => {
    seedStore({ phase: "idle" });
    renderPage("/agent?endpoint=" + REMOTE);
    expect(screen.getByTestId("agent-connect-card")).toBeTruthy();
    expect(screen.getByTestId("agent-connect")).toBeTruthy();
    expect(screen.getByTestId("agent-endpoint-row-" + REMOTE).getAttribute("aria-current")).toBe(
      "true",
    );
    expect(screen.getByTestId("agent-sessions-offline")).toBeTruthy();
  });

  it("connecting：本端点拨号中显进行时", () => {
    seedStore({ phase: "connecting", activeEndpointId: REMOTE });
    renderPage("/agent?endpoint=" + REMOTE);
    expect(screen.getByTestId("agent-connecting")).toBeTruthy();
    expect(screen.queryByTestId("agent-connect")).toBeNull();
  });

  it("connected：会话头 + Transcript + PromptComposer 三区齐备，会话行带相对时间", () => {
    const activityMs = Date.now() - 120_000;
    seedStore({
      saved: [remoteEndpoint(), remoteEndpointB()],
      phase: "online",
      activeEndpointId: REMOTE,
      activeSessionId: SESSION_A,
      sessions: [
        { sessionId: SESSION_A, title: "会话一", cwd: "/tmp/a" },
        { sessionId: SESSION_B, title: "会话二", cwd: "/tmp/b" },
      ],
      unreadByEndpoint: { [REMOTE]: 5, "ep-remote-2": 3 },
      lastInteractionByEndpoint: { [REMOTE]: activityMs },
    });
    renderPage("/agent?endpoint=" + REMOTE);
    // 中部：会话头（别名）+ transcript + composer
    expect(screen.getByTestId("agent-conversation")).toBeTruthy();
    expect(
      within(screen.getByTestId("agent-chat-conversation-pane")).getByText("远端助手"),
    ).toBeTruthy();
    expect(screen.getByTestId("acp-composer-input")).toBeTruthy();
    expect(screen.getByTestId("acp-composer-send")).toBeTruthy();
    // 侧栏：会话清单 + 选中态 + 相对时间（真实交互时刻）
    expect(screen.getByTestId("agent-session-list-" + REMOTE)).toBeTruthy();
    expect(screen.getByTestId("agent-session-row-" + SESSION_A).getAttribute("aria-current")).toBe(
      "true",
    );
    const time = screen.getByTestId("agent-session-time-" + SESSION_A);
    expect(time.textContent).toBe(formatRelative(activityMs, "zh-CN"));
    // 非当前会话没有可用时刻：不显时间，不拿端点时刻冒充
    expect(screen.queryByTestId("agent-session-time-" + SESSION_B)).toBeNull();
    // 端点行未读角标与 rail 合计同源；当前停留端点按选中清零规则不显角标
    expect(screen.getByTestId("agent-endpoint-unread-ep-remote-2").textContent).toBe("3");
    expect(screen.queryByTestId("agent-endpoint-unread-" + REMOTE)).toBeNull();
    // 新建会话入口（AF1 闭环）只在选中端点即当前连接端点时出现
    expect(screen.getByTestId("agent-sidebar-new-session")).toBeTruthy();
  });

  it("本机端点 + console 断开：引导卡显式呈现根因", () => {
    seedStore({
      saved: [localEndpoint()],
      phase: "idle",
      console: { phase: "disconnected", lastError: "pump 未就绪" },
    });
    renderPage("/agent?endpoint=" + LOCAL_ID);
    expect(screen.getByTestId("agent-console-guide")).toBeTruthy();
    expect(screen.getByTestId("agent-console-guide-error")).toBeTruthy();
  });

  it("console 快照存在时本机端点恒可选中（console 未登记也补占位行）", () => {
    seedStore({ saved: [remoteEndpoint()], console: { phase: "connecting" } });
    renderPage();
    const local = screen.getByTestId("agent-endpoint-row-" + LOCAL_ID);
    expect(local.textContent).toContain("本机 agent");
    expect(local.textContent).toContain("本机");
  });

  it("选中非连接端点时不显新建会话按钮（防误开在别的 agent 上）", () => {
    seedStore({
      phase: "online",
      activeEndpointId: "ep-other",
      activeSessionId: SESSION_A,
      sessions: [{ sessionId: SESSION_A }],
    });
    renderPage("/agent?endpoint=" + REMOTE);
    expect(screen.queryByTestId("agent-sidebar-new-session")).toBeNull();
    expect(screen.getByTestId("agent-sessions-offline")).toBeTruthy();
  });
});

describe("AgentChatPage <768 单栏互斥（与 /chat 同规则）", () => {
  it("窄屏未选端点：只显侧栏，不显对话区", () => {
    seedStore({});
    setViewportNarrow(true);
    renderPage();
    expect(screen.getByTestId("agent-endpoint-sidebar")).toBeTruthy();
    expect(screen.queryByTestId("agent-chat-conversation-pane")).toBeNull();
  });

  it("窄屏选中端点：只显对话区，返回键清选中回侧栏", () => {
    seedStore({ phase: "idle" });
    setViewportNarrow(true);
    renderPage("/agent?endpoint=" + REMOTE);
    expect(screen.queryByTestId("agent-endpoint-sidebar")).toBeNull();
    fireEvent.click(screen.getByTestId("agent-chat-back"));
    expect(screen.getByTestId("agent-endpoint-sidebar")).toBeTruthy();
    expect(screen.queryByTestId("agent-chat-conversation-pane")).toBeNull();
  });

  it("宽屏双区并存", () => {
    seedStore({ phase: "idle" });
    renderPage("/agent?endpoint=" + REMOTE);
    expect(screen.getByTestId("agent-endpoint-sidebar")).toBeTruthy();
    expect(screen.getByTestId("agent-chat-conversation-pane")).toBeTruthy();
  });
});
