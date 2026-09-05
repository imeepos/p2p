// 流程打磨轮 R3/R4/R5 测试：续连补放横幅生命周期、连接中加载态、会话焦点回退。
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WsLike } from "./ws-factory";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { AcpView } = await import("./acp-view");
const { mockAcpConsole } = await import("./mock-acp-ws");
const { useAcpStore } = await import("./acp-store");
const { renderConnected, newSession, resetFixtures } = await import("./acp-view-test-utils");
const { setWsFactory } = await import("./ws-factory");
await import("@/i18n");

beforeEach(() => {
  resetFixtures();
});

/** 打开时机由测试控制、不做任何应答的 socket：驱动 connecting 加载态 */
class ManualSocket implements WsLike {
  onopen: (() => void) | null = null;
  onclose: ((ev: { code: number; reason: string }) => void) | null = null;
  onerror: ((ev: { message?: string }) => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  send(): void {}
  close(): void {}
}

afterEach(() => {
  setWsFactory(null);
  useAcpStore.getState().disconnect();
});

describe("AcpView reattach banner lifecycle", () => {
  it("补放 0 条与 N 条文案区分：0 条仅示续连成功", async () => {
    await renderConnected();
    act(() => {
      mockAcpConsole.pushReattach(0);
    });
    const banner = await screen.findByTestId("acp-reattach-banner");
    expect(banner.textContent).toContain("已续连，无需要补放的更新");
    expect(banner.textContent).not.toContain("错过的更新");
    act(() => {
      mockAcpConsole.pushReattach(2);
    });
    await waitFor(() => {
      expect(screen.getByTestId("acp-reattach-banner").textContent).toContain("补放 2 条");
    });
  });

  it("手动关闭按钮立即移除横幅", async () => {
    await renderConnected();
    act(() => {
      mockAcpConsole.pushReattach(3);
    });
    await screen.findByTestId("acp-reattach-banner");
    fireEvent.click(screen.getByTestId("acp-reattach-dismiss"));
    await waitFor(() => {
      expect(screen.queryByTestId("acp-reattach-banner")).toBeNull();
    });
    expect(useAcpStore.getState().reattachNotice).toBeNull();
  });

  it("约 8 秒后自动消失，新通知重置计时", async () => {
    vi.useFakeTimers();
    try {
      render(<AcpView />);
      act(() => {
        useAcpStore.setState({ reattachNotice: { replayed: 1 } });
      });
      expect(screen.getByTestId("acp-reattach-banner")).toBeTruthy();
      act(() => {
        vi.advanceTimersByTime(7_999);
      });
      expect(screen.getByTestId("acp-reattach-banner")).toBeTruthy();
      act(() => {
        vi.advanceTimersByTime(1);
      });
      expect(screen.queryByTestId("acp-reattach-banner")).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("AcpView connecting indicator", () => {
  it("phase=connecting 期间主列显示加载指示，不落空态", async () => {
    setWsFactory(() => new ManualSocket());
    render(<AcpView />);
    fireEvent.click(screen.getByTestId("acp-connect"));
    const indicator = await screen.findByTestId("acp-connecting-indicator");
    expect(indicator.textContent).toContain("等待握手");
    expect(screen.queryByTestId("acp-main-empty")).toBeNull();
    useAcpStore.getState().disconnect();
    await waitFor(() => {
      expect(screen.queryByTestId("acp-connecting-indicator")).toBeNull();
    });
  });
});

describe("AcpView session focus fallback", () => {
  it("关闭当前会话后焦点自动落到列表下一个", async () => {
    await renderConnected();
    await newSession();
    fireEvent.click(screen.getByTestId("acp-session-new"));
    await screen.findByTestId("acp-session-row-s-002");
    expect(useAcpStore.getState().activeSessionId).toBe("s-002");
    fireEvent.click(screen.getByTestId("acp-session-close-s-002"));
    await screen.findByText("关闭该会话？");
    fireEvent.click(screen.getByText("关闭会话"));
    await waitFor(() => {
      expect(screen.queryByTestId("acp-session-row-s-002")).toBeNull();
    });
    expect(useAcpStore.getState().activeSessionId).toBe("s-001");
    // 主列仍处于会话模式（composer 在墙），未闪回空态
    expect(screen.getByTestId("acp-composer-input")).toBeTruthy();
    expect(screen.queryByTestId("acp-main-empty")).toBeNull();
  });

  it("关闭最后一个会话回空态", async () => {
    await renderConnected();
    await newSession();
    fireEvent.click(screen.getByTestId("acp-session-close-s-001"));
    await screen.findByText("关闭该会话？");
    fireEvent.click(screen.getByText("关闭会话"));
    await waitFor(() => {
      expect(useAcpStore.getState().activeSessionId).toBeNull();
    });
    expect(screen.getByTestId("acp-main-empty")).toBeTruthy();
  });
});

describe("AcpView online status bar", () => {
  it("online 态主列顶部显示连接状态条，随时可断开", async () => {
    await renderConnected();
    const bar = screen.getByTestId("acp-online-bar");
    expect(bar.textContent).toContain("mock-peer");
    expect(bar.textContent).toContain("断开");
    fireEvent.click(screen.getByTestId("acp-online-disconnect"));
    await waitFor(() => {
      expect(screen.getByTestId("acp-phase-badge").textContent).toContain("未连接");
    });
    expect(screen.queryByTestId("acp-online-bar")).toBeNull();
    expect(screen.getByTestId("acp-connect")).toBeTruthy();
  });
});

describe("AcpView long-turn prompt", () => {
  it("长回合超过通用 30s 超时：不误杀，chunk 续写同一气泡不碎裂", async () => {
    vi.useFakeTimers();
    try {
      mockAcpConsole.configure({
        promptScript: [
          { kind: "message", text: "长回合A" },
          { kind: "message", text: "长回合B" },
          { kind: "stop", reason: "end_turn" },
        ],
        chunkDelayMs: 31_000,
        openDelayMs: 10,
      });
      render(<AcpView />);
      fireEvent.click(screen.getByTestId("acp-connect"));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(15);
      });
      fireEvent.click(screen.getByTestId("acp-session-new"));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(5);
      });
      fireEvent.change(screen.getByTestId("acp-composer-input"), { target: { value: "hi" } });
      fireEvent.click(screen.getByTestId("acp-composer-send"));
      // 旧通用超时点（30s）：回合未结束、不得误杀
      await act(async () => {
        await vi.advanceTimersByTimeAsync(30_000);
      });
      expect(useAcpStore.getState().promptPendingBySession["s-001"]).toBe(true);
      // 第一块到达
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1_500);
      });
      expect(screen.getByTestId("acp-transcript").textContent).toContain("长回合A");
      // 第二块到达：续写同一气泡（旧代码在此处已碎成多泡）
      await act(async () => {
        await vi.advanceTimersByTimeAsync(31_000);
      });
      const bubbles = document.querySelectorAll('[data-testid^="acp-turn-assistant"]');
      expect(bubbles).toHaveLength(1);
      expect(screen.getByTestId("acp-transcript").textContent).toContain("长回合A长回合B");
      // stop 步在 93s（31s x 3）：应答帧经解码队列回填后 pending 必须复位
      await act(async () => {
        await vi.advanceTimersByTimeAsync(31_000);
      });
      expect(useAcpStore.getState().promptPendingBySession["s-001"]).toBe(false);
    } finally {
      vi.useRealTimers();
      mockAcpConsole.reset();
    }
  });
});

describe("AcpView composer lock per session", () => {
  it("turn 所在会话显示 Stop，切到其他会话恢复 Send", async () => {
    await renderConnected();
    await newSession();
    fireEvent.click(screen.getByTestId("acp-session-new"));
    await screen.findByTestId("acp-session-row-s-002");
    act(() => {
      useAcpStore.setState({ promptPendingBySession: { "s-001": true } });
    });
    // 活跃会话 s-002 无回合：Send 而非 Stop
    expect(screen.queryByTestId("acp-composer-stop")).toBeNull();
    expect(screen.getByTestId("acp-composer-send")).toBeTruthy();
    // 切回 s-001：恢复 Stop
    fireEvent.click(
      screen.getByTestId("acp-session-row-s-001").querySelector("button")!,
    );
    await waitFor(() => {
      expect(useAcpStore.getState().activeSessionId).toBe("s-001");
    });
    expect(screen.getByTestId("acp-composer-stop")).toBeTruthy();
  });

  it("Enter 发送、Shift+Enter 换行不发送", async () => {
    await renderConnected();
    await newSession();
    const input = screen.getByTestId("acp-composer-input") as HTMLTextAreaElement;
    fireEvent.change(input, { target: { value: "first" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await screen.findByText("Hello from the mock agent.");
    await waitFor(() => {
      expect(input.value).toBe("");
    });
    fireEvent.change(input, { target: { value: "draft" } });
    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    expect(input.value).toBe("draft");
    expect(useAcpStore.getState().promptPendingBySession["s-001"] ?? false).toBe(false);
  });
});
