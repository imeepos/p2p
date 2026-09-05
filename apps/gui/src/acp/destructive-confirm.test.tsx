// F5 行为测试（P2）：破坏性操作（关闭 ACP 会话、移除已保存端点、移除目录条目）
// 必须经全站确认弹框；取消则不执行。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { AcpView } = await import("./acp-view");
const { mockAcpConsole } = await import("./mock-acp-ws");
const { renderConnected, newSession, resetFixtures } = await import("./acp-view-test-utils");
await import("@/i18n");

beforeEach(() => {
  resetFixtures();
});

/** 等待当前弹框走完取消路径的收尾（Radix 关闭动画是异步的） */
async function confirmDialogActionSettled() {
  await waitFor(() => {
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });
}

describe("AcpView destructive operation confirm", () => {
  it("关闭会话：取消不关闭，确认后才关闭", async () => {
    await renderConnected();
    await newSession();
    fireEvent.click(screen.getByTestId("acp-session-close-s-001"));
    expect(await screen.findByText("关闭该会话？")).toBeTruthy();
    fireEvent.click(screen.getByText("取消"));
    await confirmDialogActionSettled();
    expect(screen.getByTestId("acp-session-row-s-001")).toBeTruthy();
    expect(mockAcpConsole.sessions.has("s-001")).toBe(true);
    // 确认路径
    fireEvent.click(screen.getByTestId("acp-session-close-s-001"));
    await screen.findByText("关闭该会话？");
    fireEvent.click(screen.getByText("关闭会话"));
    await waitFor(() => {
      expect(screen.queryByTestId("acp-session-row-s-001")).toBeNull();
    });
    expect(mockAcpConsole.sessions.has("s-001")).toBe(false);
  });

  it("移除已保存端点：取消不移除，确认后才移除", async () => {
    render(<AcpView />);
    fireEvent.click(screen.getByTestId("acp-save-endpoint"));
    expect(screen.getByTestId("acp-endpoint-fill-mock-peer")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-endpoint-remove-mock-peer"));
    expect(await screen.findByText("移除已保存端点？")).toBeTruthy();
    fireEvent.click(screen.getByText("取消"));
    await waitFor(() => {
      expect(screen.queryByRole("alertdialog")).toBeNull();
    });
    expect(screen.getByTestId("acp-endpoint-fill-mock-peer")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-endpoint-remove-mock-peer"));
    await screen.findByText("移除已保存端点？");
    fireEvent.click(screen.getByText("移除"));
    await waitFor(() => {
      expect(screen.queryByTestId("acp-endpoint-fill-mock-peer")).toBeNull();
    });
  });

  it("移除目录条目：取消不移除，确认后才移除", async () => {
    render(<AcpView />);
    fireEvent.change(screen.getByTestId("acp-directory-input"), {
      target: { value: "peer-del-1" },
    });
    fireEvent.click(screen.getByTestId("acp-directory-add"));
    expect(screen.getByTestId("acp-directory-row-peer-del-1")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-directory-remove-peer-del-1"));
    expect(await screen.findByText("移除目录条目？")).toBeTruthy();
    fireEvent.click(screen.getByText("取消"));
    await waitFor(() => {
      expect(screen.queryByRole("alertdialog")).toBeNull();
    });
    expect(screen.getByTestId("acp-directory-row-peer-del-1")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-directory-remove-peer-del-1"));
    await screen.findByText("移除目录条目？");
    fireEvent.click(screen.getByText("移除"));
    await waitFor(() => {
      expect(screen.queryByTestId("acp-directory-row-peer-del-1")).toBeNull();
    });
  });
});
