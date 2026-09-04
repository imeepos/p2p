import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { UpdateCheckResult } from "@/lib/ipc-types";

const {
  updateCheckMock,
  checkRemoteUpdateMock,
  downloadInstallMock,
  relaunchMock,
} = vi.hoisted(() => ({
  updateCheckMock: vi.fn<() => Promise<UpdateCheckResult>>(),
  checkRemoteUpdateMock: vi.fn<
    () => Promise<{ version: string; notes: string | null } | null>
  >(),
  downloadInstallMock: vi.fn<
    (onProgress: (p: { downloadedBytes: number; totalBytes: number | null }) => void) => Promise<void>
  >(),
  relaunchMock: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  ipc: { updateCheck: updateCheckMock, updateOpenReleasePage: vi.fn() },
  updateDl: {
    checkRemoteUpdate: checkRemoteUpdateMock,
    downloadAndInstallUpdate: downloadInstallMock,
    relaunchApp: relaunchMock,
  },
}));

import { AUTO_CHECK_INTERVAL_MS, useUpdateStore } from "./update-store";

function availableResult(): UpdateCheckResult {
  return {
    currentVersion: "0.1.0",
    latestVersion: "0.2.0",
    hasUpdate: true,
    releaseUrl: "https://github.com/imeepos/p2p/releases/tag/client-v0.2.0",
    releaseName: "p2p-console client-v0.2.0",
    releaseNotesMd: "## notes",
    publishedAtMs: Date.parse("2026-09-01T10:00:00Z"),
    checkedAtMs: Date.now(),
  };
}

function upToDateResult(): UpdateCheckResult {
  return {
    ...availableResult(),
    latestVersion: "0.1.0",
    hasUpdate: false,
    releaseUrl: null,
    releaseName: null,
    releaseNotesMd: null,
    publishedAtMs: null,
  };
}

async function flush(): Promise<void> {
  await vi.advanceTimersByTimeAsync(0);
}

function resetStore(): void {
  useUpdateStore.setState({
    status: "idle",
    result: null,
    error: null,
    lastSource: null,
    skippedVersion: null,
    reminderShownFor: null,
    downloadPhase: "idle",
    downloadedBytes: 0,
    totalBytes: null,
    downloadError: null,
    downloadVersion: null,
  });
}

beforeEach(() => {
  vi.useFakeTimers();
  localStorage.clear();
  updateCheckMock.mockReset();
  updateCheckMock.mockResolvedValue(upToDateResult());
  checkRemoteUpdateMock.mockReset();
  checkRemoteUpdateMock.mockResolvedValue({ version: "0.2.0", notes: null });
  downloadInstallMock.mockReset();
  downloadInstallMock.mockImplementation(async (onProgress) => {
    onProgress({ downloadedBytes: 1024, totalBytes: 4096 });
    onProgress({ downloadedBytes: 4096, totalBytes: 4096 });
  });
  relaunchMock.mockReset();
  resetStore();
});

afterEach(() => {
  useUpdateStore.getState().stopAutoCheck();
  vi.useRealTimers();
});

describe("update-store 检查三态", () => {
  it("成功有更新 → available 且 result 存档", async () => {
    updateCheckMock.mockResolvedValue(availableResult());
    await useUpdateStore.getState().check("auto");
    const s = useUpdateStore.getState();
    expect(s.status).toBe("available");
    expect(s.result?.latestVersion).toBe("0.2.0");
    expect(s.error).toBeNull();
    expect(s.lastSource).toBe("auto");
  });

  it("成功无更新 → upToDate；latestVersion 仍记录远端版本", async () => {
    await useUpdateStore.getState().check("manual");
    const s = useUpdateStore.getState();
    expect(s.status).toBe("upToDate");
    expect(s.result?.hasUpdate).toBe(false);
  });

  it("失败 → failed + error 可读；成功历史 result 保留不被清空", async () => {
    updateCheckMock.mockResolvedValueOnce(availableResult());
    await useUpdateStore.getState().check("auto");
    updateCheckMock.mockRejectedValueOnce(new Error("网络不可达"));
    await useUpdateStore.getState().check("auto");
    const s = useUpdateStore.getState();
    expect(s.status).toBe("failed");
    expect(s.error).toBe("网络不可达");
    expect(s.result?.latestVersion).toBe("0.2.0");
  });

  it("in-flight 防抖：checking 期间重复 check 被忽略", async () => {
    let resolveCheck!: (v: UpdateCheckResult) => void;
    updateCheckMock.mockImplementation(
      () =>
        new Promise<UpdateCheckResult>((resolve) => {
          resolveCheck = resolve;
        }),
    );
    const first = useUpdateStore.getState().check("manual");
    const second = useUpdateStore.getState().check("manual");
    resolveCheck(upToDateResult());
    await Promise.all([first, second]);
    expect(updateCheckMock).toHaveBeenCalledTimes(1);
  });
});

describe("update-store 下载安装", () => {
  it("下载安装成功：进度推进 → installed，不自动重启", async () => {
    updateCheckMock.mockResolvedValue(availableResult());
    await useUpdateStore.getState().check("auto");
    await useUpdateStore.getState().downloadAndInstall();
    const s = useUpdateStore.getState();
    expect(s.downloadPhase).toBe("installed");
    expect(s.downloadedBytes).toBe(4096);
    expect(s.totalBytes).toBe(4096);
    expect(s.downloadVersion).toBe("0.2.0");
    expect(downloadInstallMock).toHaveBeenCalledTimes(1);
    expect(relaunchMock).not.toHaveBeenCalled();
  });

  it("下载安装失败 → failed + 错误可读", async () => {
    updateCheckMock.mockResolvedValue(availableResult());
    await useUpdateStore.getState().check("auto");
    downloadInstallMock.mockRejectedValueOnce(new Error("签名校验失败"));
    await useUpdateStore.getState().downloadAndInstall();
    const s = useUpdateStore.getState();
    expect(s.downloadPhase).toBe("failed");
    expect(s.downloadError).toBe("签名校验失败");
  });

  it("in-flight 防抖：downloading 期间重复发起被忽略", async () => {
    updateCheckMock.mockResolvedValue(availableResult());
    await useUpdateStore.getState().check("auto");
    let resolveInstall!: () => void;
    downloadInstallMock.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveInstall = resolve;
        }),
    );
    const first = useUpdateStore.getState().downloadAndInstall();
    // flush：先让 checkRemoteUpdate 的微任务落地，进入 downloading 相位
    await flush();
    const second = useUpdateStore.getState().downloadAndInstall();
    resolveInstall();
    await Promise.all([first, second]);
    expect(downloadInstallMock).toHaveBeenCalledTimes(1);
  });

  it("非 available 态发起下载为 no-op", async () => {
    await useUpdateStore.getState().downloadAndInstall();
    expect(checkRemoteUpdateMock).not.toHaveBeenCalled();
    expect(useUpdateStore.getState().downloadPhase).toBe("idle");
  });

  it("updater 无更新句柄 → 回到 idle 不误装", async () => {
    updateCheckMock.mockResolvedValue(availableResult());
    await useUpdateStore.getState().check("auto");
    checkRemoteUpdateMock.mockResolvedValueOnce(null);
    await useUpdateStore.getState().downloadAndInstall();
    expect(downloadInstallMock).not.toHaveBeenCalled();
    expect(useUpdateStore.getState().downloadPhase).toBe("idle");
  });

  it("relaunch 由用户显式触发", () => {
    useUpdateStore.getState().relaunch();
    expect(relaunchMock).toHaveBeenCalledTimes(1);
  });
});

describe("update-store 自动轮询", () => {
  it("startAutoCheck 启动即查一次，此后每 4h 一次", async () => {
    useUpdateStore.getState().startAutoCheck();
    await flush();
    expect(updateCheckMock).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(AUTO_CHECK_INTERVAL_MS - 1);
    expect(updateCheckMock).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(updateCheckMock).toHaveBeenCalledTimes(2);
  });

  it("stopAutoCheck 清理定时器；重启不叠加轮询", async () => {
    useUpdateStore.getState().startAutoCheck();
    await flush();
    useUpdateStore.getState().stopAutoCheck();
    await vi.advanceTimersByTimeAsync(AUTO_CHECK_INTERVAL_MS * 2);
    expect(updateCheckMock).toHaveBeenCalledTimes(1);

    useUpdateStore.getState().startAutoCheck();
    useUpdateStore.getState().startAutoCheck();
    await flush();
    expect(updateCheckMock).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(AUTO_CHECK_INTERVAL_MS);
    expect(updateCheckMock).toHaveBeenCalledTimes(3);
  });
});

describe("update-store 跳过版本", () => {
  it("skipCurrentVersion 持久化 localStorage 并写入 state", async () => {
    updateCheckMock.mockResolvedValue(availableResult());
    await useUpdateStore.getState().check("manual");
    useUpdateStore.getState().skipCurrentVersion();
    const s = useUpdateStore.getState();
    expect(s.skippedVersion).toBe("0.2.0");
    expect(localStorage.getItem("p2p-console.skipped-version")).toBe("0.2.0");
  });

  it("无 result 时 skip 为 no-op，不写脏数据", () => {
    useUpdateStore.getState().skipCurrentVersion();
    expect(useUpdateStore.getState().skippedVersion).toBeNull();
    expect(localStorage.getItem("p2p-console.skipped-version")).toBeNull();
  });

  it("markReminderShown 幂等，用于 toast 去重", () => {
    const store = useUpdateStore.getState();
    store.markReminderShown("0.2.0");
    store.markReminderShown("0.2.0");
    expect(useUpdateStore.getState().reminderShownFor).toBe("0.2.0");
  });
});
