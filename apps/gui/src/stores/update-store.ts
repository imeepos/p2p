import { create } from "zustand";

import { ipc, updateDl } from "@/lib/ipc";
import type { UpdateCheckResult } from "@/lib/ipc-types";

// 契约 §9：后端无状态不轮询，节奏由前端驱动（启动后 + 每 4h + 手动）。
export const AUTO_CHECK_INTERVAL_MS = 4 * 60 * 60 * 1000;

const SKIPPED_VERSION_KEY = "p2p-console.skipped-version";

export type UpdateCheckStatus =
  | "idle"
  | "checking"
  | "upToDate"
  | "available"
  | "failed";

export type UpdateCheckSource = "auto" | "manual";

// 契约 v8 §13 下载安装相位：downloading 含进度推进；installed 后由用户决定重启时机。
export type UpdateDownloadPhase = "idle" | "downloading" | "installed" | "failed";

function readSkippedVersion(): string | null {
  try {
    return localStorage.getItem(SKIPPED_VERSION_KEY);
  } catch {
    console.warn("[update] localStorage 不可读，跳过版本仅本次会话生效");
    return null;
  }
}

function persistSkippedVersion(version: string): void {
  try {
    localStorage.setItem(SKIPPED_VERSION_KEY, version);
  } catch {
    console.warn("[update] localStorage 不可写，跳过版本仅本次会话生效");
  }
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export interface UpdateStoreState {
  status: UpdateCheckStatus;
  result: UpdateCheckResult | null;
  error: string | null;
  lastSource: UpdateCheckSource | null;
  skippedVersion: string | null;
  reminderShownFor: string | null;
  // 契约 v8 §13 下载安装状态
  downloadPhase: UpdateDownloadPhase;
  downloadedBytes: number;
  totalBytes: number | null;
  downloadError: string | null;
  downloadVersion: string | null;
  check: (source: UpdateCheckSource) => Promise<void>;
  skipCurrentVersion: () => void;
  markReminderShown: (version: string) => void;
  startAutoCheck: () => void;
  stopAutoCheck: () => void;
  downloadAndInstall: () => Promise<void>;
  relaunch: () => void;
}

// 轮询定时器挂在 store 模块层而非组件 effect：幂等启停，
// 组件卸载/重复挂载（StrictMode 双执行）不泄漏不叠加。
let autoTimer: number | null = null;

export const useUpdateStore = create<UpdateStoreState>()((set, get) => ({
  status: "idle",
  result: null,
  error: null,
  lastSource: null,
  skippedVersion: readSkippedVersion(),
  reminderShownFor: null,
  downloadPhase: "idle",
  downloadedBytes: 0,
  totalBytes: null,
  downloadError: null,
  downloadVersion: null,

  check: async (source) => {
    // in-flight 防抖：StrictMode 双挂载与轮询重叠只保留一次检查
    if (get().status === "checking") return;
    set({ status: "checking", error: null, lastSource: source });
    try {
      const result = await ipc.updateCheck();
      set({ status: result.hasUpdate ? "available" : "upToDate", result });
    } catch (error) {
      // 失败可观测：自动检查静默落状态（设置段可见），不打扰；
      // 手动检查的打扰由视图层按 lastSource 决定。
      console.error("[update] update_check 失败", error);
      set({ status: "failed", error: toErrorMessage(error) });
    }
  },

  skipCurrentVersion: () => {
    const version = get().result?.latestVersion;
    if (!version) return;
    persistSkippedVersion(version);
    set({ skippedVersion: version });
  },

  markReminderShown: (version) => {
    if (get().reminderShownFor === version) return;
    set({ reminderShownFor: version });
  },

  // 下载安装（契约 v8 §13）：仅 available 态可发起；in-flight 防抖与 check 同思路。
  // 先经 updater 端点重新拿取更新句柄（避免跨轮询周期持旧句柄），再一并完成下载+安装。
  downloadAndInstall: async () => {
    if (get().status !== "available") return;
    const phase = get().downloadPhase;
    if (phase === "downloading") return;
    set({
      downloadPhase: "downloading",
      downloadError: null,
      downloadedBytes: 0,
      totalBytes: null,
      downloadVersion: get().result?.latestVersion ?? null,
    });
    try {
      const remote = await updateDl.checkRemoteUpdate();
      if (!remote) {
        set({ downloadPhase: "idle", downloadVersion: null });
        return;
      }
      await updateDl.downloadAndInstallUpdate((p) =>
        set({ downloadedBytes: p.downloadedBytes, totalBytes: p.totalBytes }),
      );
      set({ downloadPhase: "installed" });
    } catch (error) {
      // 失败可观测：状态落 store 由视图展示，console 留痕（禁静默吞）
      console.error("[update] 下载安装失败", error);
      set({ downloadPhase: "failed", downloadError: toErrorMessage(error) });
    }
  },

  relaunch: () => {
    void updateDl.relaunchApp();
  },

  startAutoCheck: () => {
    if (autoTimer !== null) return;
    void get().check("auto");
    autoTimer = window.setInterval(() => {
      void get().check("auto");
    }, AUTO_CHECK_INTERVAL_MS);
  },

  stopAutoCheck: () => {
    if (autoTimer === null) return;
    window.clearInterval(autoTimer);
    autoTimer = null;
  },
}));
