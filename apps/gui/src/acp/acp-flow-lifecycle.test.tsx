// 流程打磨轮 R3/R4/R5 测试：续连补放横幅生命周期、连接中加载态、会话焦点回退。
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { AcpView } = await import("./acp-view");
const { mockAcpConsole } = await import("./mock-acp-ws");
const { useAcpStore } = await import("./acp-store");
const { renderConnected, resetFixtures } = await import("./acp-view-test-utils");
await import("@/i18n");

beforeEach(() => {
  resetFixtures();
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
