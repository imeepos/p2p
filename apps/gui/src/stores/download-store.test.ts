import { beforeEach, describe, expect, it, vi } from "vitest";

const { exportMediaMock } = vi.hoisted(() => ({
  exportMediaMock: vi.fn<
    (
      sourceUrl: string,
      destPath: string,
      onProgress: (p: { receivedBytes: number; totalBytes: number }) => void,
    ) => Promise<{ destPath: string; totalBytes: number }>
  >(),
}));

vi.mock("@/lib/ipc", () => ({
  mediaDl: { pickSavePath: vi.fn(), exportMedia: exportMediaMock },
}));

import { useDownloadStore } from "./download-store";

const INPUT = {
  key: "asset://localhost/k1",
  sourceUrl: "asset://localhost/k1",
  name: "a.bin",
  destPath: "/tmp/a.bin",
};

beforeEach(() => {
  exportMediaMock.mockReset();
  useDownloadStore.setState({ tasks: {} });
});

describe("download-store 媒体导出", () => {
  it("进度推进 → done，字节随进度更新，终态以命令回执为准", async () => {
    exportMediaMock.mockImplementation(async (_src, _dest, onProgress) => {
      onProgress({ receivedBytes: 256 * 1024, totalBytes: 300_000 });
      return { destPath: INPUT.destPath, totalBytes: 300_000 };
    });
    await useDownloadStore.getState().startMediaExport(INPUT);
    const task = useDownloadStore.getState().tasks[INPUT.key];
    expect(task.phase).toBe("done");
    expect(task.receivedBytes).toBe(256 * 1024);
    expect(task.totalBytes).toBe(300_000);
    expect(task.error).toBeNull();
  });

  it("复制中同 key 重复发起为 no-op（in-flight 防抖）", async () => {
    let release: () => void = () => {};
    exportMediaMock.mockImplementation(
      () =>
        new Promise<{ destPath: string; totalBytes: number }>((resolve) => {
          release = () => resolve({ destPath: INPUT.destPath, totalBytes: 0 });
        }),
    );
    const first = useDownloadStore.getState().startMediaExport(INPUT);
    await useDownloadStore.getState().startMediaExport(INPUT);
    expect(exportMediaMock).toHaveBeenCalledTimes(1);
    release();
    await first;
    expect(useDownloadStore.getState().tasks[INPUT.key].phase).toBe("done");
  });

  it("失败 → failed + error 可读，console 留痕", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    exportMediaMock.mockRejectedValueOnce(new Error("磁盘已满"));
    await useDownloadStore.getState().startMediaExport(INPUT);
    const task = useDownloadStore.getState().tasks[INPUT.key];
    expect(task.phase).toBe("failed");
    expect(task.error).toBe("磁盘已满");
    expect(errorSpy).toHaveBeenCalled();
    errorSpy.mockRestore();
  });

  it("失败后可重新发起（重试走 done）", async () => {
    exportMediaMock.mockRejectedValueOnce(new Error("x"));
    exportMediaMock.mockResolvedValueOnce({ destPath: INPUT.destPath, totalBytes: 1 });
    await useDownloadStore.getState().startMediaExport(INPUT);
    await useDownloadStore.getState().startMediaExport(INPUT);
    expect(useDownloadStore.getState().tasks[INPUT.key].phase).toBe("done");
    expect(exportMediaMock).toHaveBeenCalledTimes(2);
  });
});
