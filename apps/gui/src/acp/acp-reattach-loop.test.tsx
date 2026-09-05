// 目标一 stub 回环（验收 D，stub 模式；真机 acp_wave_e2e 双模式不具备，
// 以 mock WS 工厂脚本化桥行为、fetch stub 模拟 console /reattach）：
// 断流 → 自动重连前查 console 票据 → 握手行携 reattach → 收到
// dsh/bridge/reattach 且补放计数 > 0；窗口过期分支给原会话失效引导。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAcpStore } from "./acp-store";
import { createMockWsFactory, mockAcpConsole, type MockSocket } from "./mock-acp-ws";
import { setWsFactory, type WebSocketFactory, type WsLike } from "./ws-factory";
import { AcpView } from "./acp-view";

await import("@/i18n");

let urls: string[] = [];
let fetchMock: ReturnType<typeof vi.fn>;

function stubConsoleReattach(body: unknown): void {
  fetchMock = vi.fn(async (url: string) => {
    if (String(url).includes("/reattach")) return { ok: true, json: async () => body };
    return { ok: true, json: async () => ({ peers: [] }) };
  });
  vi.stubGlobal("fetch", fetchMock);
}

function bridgeFactory(): WebSocketFactory {
  const mockFactory = createMockWsFactory();
  return (url: string): WsLike => {
    urls.push(url);
    const socket = mockFactory(url) as WsLike & MockSocket;
    if (url.includes("reattach=")) {
      // 桥约定：重连后客户端 initialize 过桥，桥先补放 dsh/bridge/reattach 通知。
      // stub 挂在首条客户端行上（factory 返回时客户端尚未挂 onopen，不能劫持它）。
      const originalSend = socket.send.bind(socket);
      let pushed = false;
      socket.send = (data: string) => {
        originalSend(data);
        if (!pushed) {
          pushed = true;
          setTimeout(() => socket.serverPush("dsh/bridge/reattach", { replayed: 3 }), 20);
        }
      };
    }
    return socket;
  };
}

const DRAFT = {
  wsUrl: "ws://127.0.0.1:8787",
  token: "mock-token",
  peer: "mock-peer",
  statusUrl: "http://127.0.0.1:9900",
};

beforeEach(() => {
  localStorage.clear();
  mockAcpConsole.reset();
  useAcpStore.getState().resetConsoleState();
  useAcpStore.setState({ draft: { ...DRAFT } });
  urls = [];
  setWsFactory(bridgeFactory());
});

afterEach(() => {
  setWsFactory(null);
  vi.unstubAllGlobals();
});

describe("续连回环（stub 模式，验收 D）", () => {
  it("断流 → 窗口内携票据重连 → 收到补放通知且计数 > 0", async () => {
    stubConsoleReattach({
      peer: "mock-peer",
      ticket: "tk-loop-3",
      expires_at_unix_ms: Date.now() + 60_000,
      reason: "ok",
    });
    render(<AcpView />);
    fireEvent.click(screen.getByTestId("acp-connect"));
    await screen.findByTestId("acp-capabilities-card");

    // 对端断流 → 自动重连：重连拨号行携带 console 查到的票据
    mockAcpConsole.dropAll(1000, "agent-stream-dropped");
    await waitFor(
      () => {
        expect(urls.some((u) => u.includes("reattach=tk-loop-3"))).toBe(true);
      },
      { timeout: 8000 },
    );

    // 桥补放通知到达：横幅显示补放 3 条（>0），不显示原会话失效引导
    await waitFor(
      () => {
        expect(screen.getByTestId("acp-reattach-banner").textContent).toContain("补放 3 条");
      },
      { timeout: 8000 },
    );
    expect(useAcpStore.getState().reattachNotice).toEqual({ replayed: 3 });
    expect(useAcpStore.getState().sessionLostNotice).toBe(false);
    expect(fetchMock.mock.calls.some(([u]) => String(u).includes("/reattach"))).toBe(true);
  });

  it("窗口过期（reason=expired）→ fresh 重连 + 原会话失效引导可关闭", async () => {
    stubConsoleReattach({ peer: "mock-peer", ticket: null, reason: "expired" });
    render(<AcpView />);
    fireEvent.click(screen.getByTestId("acp-connect"));
    await screen.findByTestId("acp-capabilities-card");
    mockAcpConsole.dropAll(1000, "agent-stream-dropped");

    // 重连拨号不带 reattach（fresh），页面给原会话失效引导
    await waitFor(
      () => {
        expect(urls.length).toBeGreaterThanOrEqual(2);
        expect(urls[1]).not.toContain("reattach=");
      },
      { timeout: 8000 },
    );
    const notice = await screen.findByTestId("acp-session-lost-notice");
    expect(notice.textContent).toContain("原会话已失效");
    fireEvent.click(screen.getByTestId("acp-session-lost-dismiss"));
    expect(screen.queryByTestId("acp-session-lost-notice")).toBeNull();
  });
});
