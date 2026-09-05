// 流程打磨轮 R3/R4/R5 测试：续连补放横幅生命周期、连接中加载态、会话焦点回退。
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WsLike } from "./ws-factory";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { AcpView } = await import("./acp-view");
const { mockAcpConsole } = await import("./mock-acp-ws");
const { useAcpStore } = await import("./acp-store");
const { renderConnected, resetFixtures } = await import("./acp-view-test-utils");
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
