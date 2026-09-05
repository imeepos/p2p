import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

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
