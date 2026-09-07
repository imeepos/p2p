import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatMediaJson } from "@/lib/ipc-types";

const mocks = vi.hoisted(() => ({
  isTauriMock: vi.fn<() => boolean>(),
  pickSavePathMock: vi.fn<(name: string) => Promise<string | null>>(),
  exportMediaMock: vi.fn<
    (
      sourceUrl: string,
      destPath: string,
      onProgress: (p: { receivedBytes: number; totalBytes: number }) => void,
    ) => Promise<{ destPath: string; totalBytes: number }>
  >(),
  toastSuccessMock: vi.fn(),
  toastErrorMock: vi.fn(),
}));

vi.mock("@/lib/tauri-env", () => ({ isTauriRuntime: mocks.isTauriMock }));
vi.mock("@/lib/ipc", () => ({
  mediaDl: { pickSavePath: mocks.pickSavePathMock, exportMedia: mocks.exportMediaMock },
}));
vi.mock("@/components/feedback/toast", () => ({
  toastSuccess: mocks.toastSuccessMock,
  toastError: mocks.toastErrorMock,
}));

import { useDownloadStore } from "@/stores/download-store";

import { MediaDownload } from "./media-download";
import "@/i18n";

const MEDIA: ChatMediaJson = {
  name: "报告.pdf",
  mime: "application/pdf",
  size: 3,
  path: "asset://localhost/report.pdf",
};

const DEST = "/Users/me/Desktop/报告.pdf";

beforeEach(() => {
  useDownloadStore.setState({ tasks: {} });
  mocks.isTauriMock.mockReset().mockReturnValue(true);
  mocks.pickSavePathMock.mockReset();
  mocks.exportMediaMock.mockReset();
  mocks.toastSuccessMock.mockReset();
  mocks.toastErrorMock.mockReset();
});

afterEach(() => cleanup());

describe("MediaDownload 桌面壳导出流", () => {
  it("点击下载先弹保存对话框，默认文件名为附件原始名", async () => {
    mocks.pickSavePathMock.mockResolvedValue(null);
    render(<MediaDownload media={MEDIA} />);
    fireEvent.click(screen.getByTestId("media-download-button"));
    await waitFor(() => expect(mocks.pickSavePathMock).toHaveBeenCalledWith("报告.pdf"));
    expect(mocks.exportMediaMock).not.toHaveBeenCalled();
  });

  it("取消对话框不发导出命令", async () => {
    mocks.pickSavePathMock.mockResolvedValue(null);
    render(<MediaDownload media={MEDIA} />);
    fireEvent.click(screen.getByTestId("media-download-button"));
    await waitFor(() => expect(mocks.pickSavePathMock).toHaveBeenCalled());
    expect(useDownloadStore.getState().tasks[MEDIA.path as string]).toBeUndefined();
  });

  it("选定目标后后台导出，进度条渲染，完成 toast 带目标路径", async () => {
    mocks.pickSavePathMock.mockResolvedValue(DEST);
    mocks.exportMediaMock.mockImplementation((_src, _dest, onProgress) => {
      onProgress({ receivedBytes: 262_144, totalBytes: 524_288 });
      return Promise.resolve({ destPath: DEST, totalBytes: 524_288 });
    });
    render(<MediaDownload media={MEDIA} />);
    fireEvent.click(screen.getByTestId("media-download-button"));
    // 进度条随 store 推进上屏
    await screen.findByRole("progressbar");
    expect(mocks.exportMediaMock).toHaveBeenCalledWith(
      MEDIA.path,
      DEST,
      expect.any(Function),
    );
    await waitFor(() =>
      expect(useDownloadStore.getState().tasks[MEDIA.path as string]?.phase).toBe("done"),
    );
    expect(mocks.toastSuccessMock).toHaveBeenCalledWith("已保存", DEST);
    expect(screen.getByTestId("media-download-done")).toBeTruthy();
  });

  it("导出失败 → 失败标记 + toast + 重试可再次发起", async () => {
    mocks.pickSavePathMock.mockResolvedValue(DEST);
    mocks.exportMediaMock.mockRejectedValueOnce(new Error("写入目标文件失败"));
    render(<MediaDownload media={MEDIA} />);
    fireEvent.click(screen.getByTestId("media-download-button"));
    await waitFor(() =>
      expect(useDownloadStore.getState().tasks[MEDIA.path as string]?.phase).toBe("failed"),
    );
    expect(mocks.toastErrorMock).toHaveBeenCalled();
    expect(screen.getByTestId("media-download-failed").textContent).toContain("写入目标文件失败");

    mocks.exportMediaMock.mockResolvedValueOnce({ destPath: DEST, totalBytes: 1 });
    fireEvent.click(screen.getByTestId("media-download-retry"));
    await waitFor(() =>
      expect(useDownloadStore.getState().tasks[MEDIA.path as string]?.phase).toBe("done"),
    );
  });

  it("复制中按钮禁用且不重复弹对话框", async () => {
    let release: () => void = () => {};
    mocks.pickSavePathMock.mockResolvedValue(DEST);
    mocks.exportMediaMock.mockImplementation(
      () =>
        new Promise<{ destPath: string; totalBytes: number }>((resolve) => {
          release = () => resolve({ destPath: DEST, totalBytes: 0 });
        }),
    );
    render(<MediaDownload media={MEDIA} />);
    fireEvent.click(screen.getByTestId("media-download-button"));
    await screen.findByRole("progressbar");
    expect(
      (screen.getByTestId("media-download-button") as HTMLButtonElement).disabled,
    ).toBe(true);
    fireEvent.click(screen.getByTestId("media-download-button"));
    expect(mocks.pickSavePathMock).toHaveBeenCalledTimes(1);
    release();
  });
});

describe("MediaDownload 非 Tauri 环境兜底", () => {
  it("浏览器/jsdom 保留锚点直下", () => {
    mocks.isTauriMock.mockReturnValue(false);
    render(<MediaDownload media={MEDIA} />);
    const anchor = screen.getByRole("link", { name: "下载" });
    expect(anchor.getAttribute("href")).toBe(MEDIA.path);
    expect(anchor.getAttribute("download")).toBe("报告.pdf");
  });

  it("无 path 的占位媒体不渲染任何下载入口", () => {
    render(
      <MediaDownload media={{ name: "m.bin", mime: "application/octet-stream", size: 1, path: null }} />,
    );
    expect(screen.queryByTestId("media-download")).toBeNull();
    expect(screen.queryByRole("link")).toBeNull();
  });
});
