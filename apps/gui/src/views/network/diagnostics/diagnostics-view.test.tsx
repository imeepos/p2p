import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const logClearMock = vi.fn(async () => {});
const logTailMock = vi.fn(async (_maxLines?: number) => ["line-1", "line-2"]);
const logPathMock = vi.fn(async () => "/tmp/frontend.log");
vi.mock("@/lib/ipc", () => ({
  diag: {
    logClear: () => logClearMock(),
    logTail: (n?: number) => logTailMock(n),
    logPath: () => logPathMock(),
  },
}));

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import "@/i18n";
import {
  clearRecentErrors,
  installErrorReport,
} from "@/lib/error-report";
import { DiagnosticsView } from "./diagnostics-view";

function renderView() {
  return render(
    <ConfirmProvider>
      <DiagnosticsView />
    </ConfirmProvider>,
  );
}

beforeEach(() => {
  logClearMock.mockClear();
  logTailMock.mockClear();
  logPathMock.mockClear();
  logTailMock.mockResolvedValue(["line-1"]);
  logPathMock.mockResolvedValue("/tmp/frontend.log");
  // 诊断 IPC 仅在 Tauri 桥下走（F27）；桌面用例默认注入运行时标记
  window.__TAURI_INTERNALS__ = {};
  clearRecentErrors();
});

afterEach(() => {
  delete window.__TAURI_INTERNALS__;
});

describe("DiagnosticsView 一键清理（需求 5）", () => {
  it("点击清理先弹确认框，说明会删除日志文件，此时不执行清理", async () => {
    renderView();
    fireEvent.click(await screen.findByText("一键清理诊断数据"));
    expect(await screen.findByText("清理诊断数据？")).toBeTruthy();
    expect(screen.getByText(/删除持久化日志文件/)).toBeTruthy();
    expect(logClearMock).not.toHaveBeenCalled();
  });

  it("确认框取消：不清理，缓冲与日志保持原样", async () => {
    renderView();
    fireEvent.click(await screen.findByText("一键清理诊断数据"));
    fireEvent.click(await screen.findByText("取消"));
    await waitFor(() => {
      expect(screen.queryByText("清理诊断数据？")).toBeNull();
    });
    expect(logClearMock).not.toHaveBeenCalled();
    expect(screen.getByText("line-1")).toBeTruthy();
  });

  it("确认清理：logClear 被调用，日志尾部清空", async () => {
    renderView();
    fireEvent.click(await screen.findByText("一键清理诊断数据"));
    fireEvent.click(await screen.findByText("清理"));
    await waitFor(() => {
      expect(logClearMock).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(screen.queryByText("line-1")).toBeNull();
    });
  });
});

describe("DiagnosticsView 非 Tauri 降级（F27）", () => {
  it("浏览器态：不发诊断 IPC，环境卡与日志尾显示桌面端说明", async () => {
    delete window.__TAURI_INTERNALS__;
    renderView();
    await waitFor(() => {
      expect(screen.getByTestId("diagnostics-desktop-only-env")).toBeTruthy();
      expect(screen.getByTestId("diagnostics-desktop-only-tail")).toBeTruthy();
    });
    expect(screen.getByTestId("diagnostics-desktop-only-env").textContent).toContain(
      "桌面端",
    );
    expect(logPathMock).not.toHaveBeenCalled();
    expect(logTailMock).not.toHaveBeenCalled();
  });

  it("最近错误堆栈折叠为「复制详情」，不再直出原始堆栈", async () => {
    // 经 window error 事件注入（console.error 已被 installErrorReport 包装，
    // 测试内再 spy 会把记录器替换掉，缓冲不会增长）
    installErrorReport();
    const seeded = Object.assign(new Error("boom"), { stack: "Error: boom\n  at f" });
    window.dispatchEvent(new ErrorEvent("error", { message: "boom", error: seeded }));
    renderView();
    const copyBtn = await screen.findByTestId("diagnostics-error-copy-0");
    expect(screen.queryByText(/at f/)).toBeNull();
    const writeText = vi.fn<(text: string) => Promise<void>>().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    fireEvent.click(copyBtn);
    await waitFor(() => expect(writeText).toHaveBeenCalled());
    expect(writeText.mock.calls[0]?.[0]).toContain("boom");
    expect(writeText.mock.calls[0]?.[0]).toContain("at f");
  });
});
