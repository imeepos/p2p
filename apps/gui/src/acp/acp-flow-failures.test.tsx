// 流程打磨轮 R1/R2 测试：动作失败 toast 可见化、手动立即重试与 offline 终态折射。
// 全链走注入 socket（打开即在线、请求一律错误），不依赖 mock 会话成功路径。
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Toaster, toast } from "sonner";
import type { WsLike, WebSocketFactory } from "./ws-factory";
import type { AcpConnectionEvents, ReconnectPolicy } from "./acp-connection";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { AcpView } = await import("./acp-view");
const { mockAcpConsole } = await import("./mock-acp-ws");
const { useAcpStore } = await import("./acp-store");
const { resetFixtures } = await import("./acp-view-test-utils");
const { setWsFactory } = await import("./ws-factory");
const { AcpConnection } = await import("./acp-connection");
await import("@/i18n");

beforeEach(() => {
  resetFixtures();
});

afterEach(() => {
  setWsFactory(null);
  useAcpStore.getState().disconnect();
  act(() => {
    toast.dismiss();
  });
});

/** 打开时机由测试手动控制、请求一律错误的 socket：retryNow 单测用 */
class ManualSocket implements WsLike {
  onopen: (() => void) | null = null;
  onclose: ((ev: { code: number; reason: string }) => void) | null = null;
  onerror: ((ev: { message?: string }) => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  send(): void {}
  close(): void {}
}

/** 打开即在线、但对一切请求回 JSON-RPC 错误的 socket：驱动全部动作失败路径 */
class RejectingSocket implements WsLike {
  onopen: (() => void) | null = null;
  onclose: ((ev: { code: number; reason: string }) => void) | null = null;
  onerror: ((ev: { message?: string }) => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  constructor() {
    window.setTimeout(() => this.onopen?.(), 0);
  }
  send(data: string): void {
    const msg = JSON.parse(data) as { id?: number };
    if (typeof msg.id !== "number") return;
    this.onmessage?.({
      data: new TextEncoder().encode(
        JSON.stringify({
          jsonrpc: "2.0",
          id: msg.id,
          error: { code: -32000, message: "mock rejected" },
        }) + "\n",
      ),
    });
  }
  close(): void {
    this.onclose?.({ code: 1000, reason: "client-close" });
  }
}

async function renderWithRejectingSocket() {
  setWsFactory(() => new RejectingSocket());
  render(
    <>
      <AcpView />
      <Toaster position="top-right" />
    </>,
  );
  fireEvent.click(screen.getByTestId("acp-connect"));
  // online 后连接卡卸载、徽章消失；会话侧栏新建按钮随 online 解禁
  await waitFor(() => {
    expect(screen.getByTestId("acp-session-new")).not.toBeDisabled();
  });
}

describe("AcpConnection retryNow", () => {
  const POLICY: ReconnectPolicy = { maxAttempts: 2, baseDelayMs: 5, maxDelayMs: 10 };

  function harness() {
    const sockets: ManualSocket[] = [];
    const factory: WebSocketFactory = () => {
      const socket = new ManualSocket();
      sockets.push(socket);
      return socket;
    };
    const phases: string[] = [];
    const reconnects: number[] = [];
    const events: AcpConnectionEvents = {
      onPhase: (p) => phases.push(p),
      onNotification: () => {},
      onRequest: () => {},
      onCloseInfo: () => {},
      onReconnect: (attempt) => reconnects.push(attempt),
    };
    const conn = new AcpConnection(
      { wsUrl: "ws://127.0.0.1:1", token: "t", peer: "p" },
      factory,
      events,
      POLICY,
    );
    return { conn, sockets, phases, reconnects };
  }

  it("手动重试立即重拨：不等退避定时器，phase 走 connecting", () => {
    const h = harness();
    h.conn.connect();
    h.sockets[h.sockets.length - 1].onopen?.();
    h.sockets[0].onclose?.({ code: 1000, reason: "dropped" });
    expect(h.reconnects).toEqual([1]);
    h.conn.retryNow();
    expect(h.phases[h.phases.length - 1]).toBe("connecting");
    expect(h.sockets).toHaveLength(2);
  });

  it("手动重试后再次断流按既有计数继续调度（attempt 2）", async () => {
    const h = harness();
    h.conn.connect();
    h.sockets[h.sockets.length - 1].onopen?.();
    h.sockets[0].onclose?.({ code: 1000, reason: "dropped" });
    h.conn.retryNow();
    // 手动重试的拨号未及 open 即断：计数不打断，落到 attempt 2 并按退避调度
    h.sockets[1].onclose?.({ code: 1000, reason: "dropped" });
    expect(h.reconnects).toEqual([1, 2]);
    await new Promise((r) => setTimeout(r, 15));
    expect(h.sockets).toHaveLength(3);
  });

  it("用户主动断开后 retryNow 不拨号", () => {
    const h = harness();
    h.conn.connect();
    h.sockets[h.sockets.length - 1].onopen?.();
    h.conn.close();
    h.conn.retryNow();
    expect(h.sockets).toHaveLength(1);
    expect(h.phases[h.phases.length - 1]).toBe("idle");
  });
});

describe("AcpView action failure visibility", () => {
  it("initialize 握手失败弹 toast，console.warn 留痕不删", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      await renderWithRejectingSocket();
      expect(await screen.findByText("initialize 失败，能力信息不可用")).toBeTruthy();
      expect(warnSpy.mock.calls.some((c) => String(c[0]).includes("initialize 失败"))).toBe(true);
    } finally {
      warnSpy.mockRestore();
    }
  });

  it("online 下新建会话失败弹 toast（页面可见）", async () => {
    await renderWithRejectingSocket();
    fireEvent.click(screen.getByTestId("acp-session-new"));
    expect(await screen.findByText("新建会话失败")).toBeTruthy();
  });

  it("online 下 prompt 发送失败弹 toast 且 pending 复位", async () => {
    await renderWithRejectingSocket();
    act(() => {
      useAcpStore.setState({ activeSessionId: "s-manual" });
    });
    fireEvent.change(screen.getByTestId("acp-composer-input"), { target: { value: "hi" } });
    fireEvent.click(screen.getByTestId("acp-composer-send"));
    expect(await screen.findByText("Prompt 发送失败")).toBeTruthy();
    expect(useAcpStore.getState().promptPending).toBe(false);
  });

  it("online 下恢复与关闭会话失败弹 toast", async () => {
    await renderWithRejectingSocket();
    act(() => {
      useAcpStore.setState({ sessions: [{ sessionId: "s-x", title: "X" }] });
    });
    fireEvent.click(screen.getByTestId("acp-session-row-s-x").querySelector("button")!);
    expect(await screen.findByText("恢复会话失败")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-session-close-s-x"));
    expect(await screen.findByText("关闭会话失败")).toBeTruthy();
  });

  it("online 下配置下发失败弹 toast", async () => {
    await renderWithRejectingSocket();
    act(() => {
      useAcpStore.setState({ activeSessionId: "s-manual" });
    });
    await useAcpStore.getState().setConfigOption("model", "mock-model-b");
    expect(await screen.findByText("配置下发失败")).toBeTruthy();
  });
});

describe("AcpView reconnect retry", () => {
  it("重连横幅带立即重试按钮：点击立即回在线", async () => {
    const { renderConnected } = await import("./acp-view-test-utils");
    await renderConnected();
    act(() => {
      mockAcpConsole.dropAll();
    });
    const banner = await screen.findByTestId("acp-reconnect-banner");
    expect(banner.textContent).toContain("立即重试");
    fireEvent.click(screen.getByTestId("acp-reconnect-now"));
    await waitFor(
      () => {
        expect(screen.getByTestId("acp-phase-badge").textContent).toContain("在线");
      },
      { timeout: 5_000 },
    );
    expect(screen.queryByTestId("acp-reconnect-banner")).toBeNull();
  });

  it("offline 终态：失败原因与重试入口在连接区可见", async () => {
    mockAcpConsole.configure({ deniedPeers: ["denied-peer"] });
    useAcpStore.getState().setDraft({ peer: "denied-peer" });
    render(
      <>
        <AcpView />
        <Toaster position="top-right" />
      </>,
    );
    fireEvent.click(screen.getByTestId("acp-connect"));
    const reason = await screen.findByTestId("acp-offline-reason");
    expect(reason.textContent).toContain("鉴权被拒绝");
    fireEvent.click(screen.getByTestId("acp-retry-connect"));
    await waitFor(() => {
      expect(screen.getByTestId("acp-phase-badge").textContent).toContain("离线");
    });
    expect(screen.getByTestId("acp-offline-reason")).toBeTruthy();
  });
});
