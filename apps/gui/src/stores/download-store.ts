import { create } from "zustand";

import { mediaDl } from "@/lib/ipc";
import type { MediaExportProgressPayload } from "@/lib/ipc-types";

// 媒体导出任务相位：copying 含进度推进；done/failed 终态留档供气泡展示。
// 任务表全局常驻（key = 附件 asset URL），切会话/切页面进度不丢（§12.5 后台导出）。
export type MediaDownloadPhase = "copying" | "done" | "failed";

export interface MediaDownloadTask {
  key: string;
  name: string;
  destPath: string;
  receivedBytes: number;
  totalBytes: number;
  phase: MediaDownloadPhase;
  error: string | null;
}

interface StartMediaExportInput {
  key: string;
  sourceUrl: string;
  name: string;
  destPath: string;
}

interface DownloadStoreState {
  tasks: Record<string, MediaDownloadTask>;
  /** 发起后台导出；同 key 复制中重复发起为 no-op（in-flight 防抖）。 */
  startMediaExport: (input: StartMediaExportInput) => Promise<void>;
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export const useDownloadStore = create<DownloadStoreState>()((set, get) => ({
  tasks: {},

  startMediaExport: async ({ key, sourceUrl, name, destPath }) => {
    if (get().tasks[key]?.phase === "copying") return;
    set((s) => ({
      tasks: {
        ...s.tasks,
        [key]: {
          key,
          name,
          destPath,
          receivedBytes: 0,
          totalBytes: 0,
          phase: "copying",
          error: null,
        },
      },
    }));
    try {
      const result = await mediaDl.exportMedia(
        sourceUrl,
        destPath,
        (p: MediaExportProgressPayload) => {
          set((s) => {
            const task = s.tasks[key];
            if (!task || task.phase !== "copying") return s;
            return {
              tasks: {
                ...s.tasks,
                [key]: { ...task, receivedBytes: p.receivedBytes, totalBytes: p.totalBytes },
              },
            };
          });
        },
      );
      set((s) => {
        const task = s.tasks[key];
        if (!task) return s;
        return {
          tasks: { ...s.tasks, [key]: { ...task, phase: "done", totalBytes: result.totalBytes } },
        };
      });
    } catch (error) {
      // 失败可观测：状态落 store 由气泡/toast 展示，console 留痕（禁静默吞）
      console.error("[media-export] 附件导出失败", error);
      set((s) => {
        const task = s.tasks[key];
        if (!task) return s;
        return {
          tasks: {
            ...s.tasks,
            [key]: { ...task, phase: "failed", error: toErrorMessage(error) },
          },
        };
      });
    }
  },
}));
