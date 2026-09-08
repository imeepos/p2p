import { useTranslation } from "react-i18next";

import { DownloadProgress } from "@/components/feedback/download-progress";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { mediaDl } from "@/lib/ipc";
import type { ChatMediaJson } from "@/lib/ipc-types";
import { isTauriRuntime } from "@/lib/tauri-env";
import { useDownloadStore } from "@/stores/download-store";

// 媒体附件下载入口（契约 §12.5）：桌面壳走保存对话框 + 后台导出，进度内联展示；
// 浏览器预览/jsdom 无对话框，保留锚点直下兜底（与旧行为一致）。
export function MediaDownload({ media }: { media: ChatMediaJson }) {
  const { t } = useTranslation();
  const sourceUrl = media.path ?? null;
  const task = useDownloadStore((s) => (sourceUrl ? s.tasks[sourceUrl] : null));

  if (!sourceUrl) return null;

  const saving = task?.phase === "copying";
  const pickAndExport = async () => {
    if (saving) return;
    const destPath = await mediaDl.pickSavePath(media.name);
    // 用户取消保存对话框则不发起导出
    if (!destPath) return;
    await useDownloadStore
      .getState()
      .startMediaExport({ key: sourceUrl, sourceUrl, name: media.name, destPath });
    const phase = useDownloadStore.getState().tasks[sourceUrl]?.phase;
    if (phase === "done") {
      toastSuccess(t("chat.downloadSaved"), destPath);
    } else if (phase === "failed") {
      const error = useDownloadStore.getState().tasks[sourceUrl]?.error;
      toastError(t("chat.downloadFailed"), { description: error ?? undefined });
    }
  };

  if (!isTauriRuntime()) {
    return (
      <a href={sourceUrl} download={media.name} className="ml-2 underline underline-offset-2">
        {t("chat.download")}
      </a>
    );
  }

  return (
    <span className="mt-1 block" data-testid="media-download">
      <button
        type="button"
        data-testid="media-download-button"
        disabled={saving}
        onClick={() => void pickAndExport()}
        className="underline underline-offset-2 disabled:opacity-60"
      >
        {saving ? t("chat.downloadSaving") : t("chat.download")}
      </button>
      {task?.phase === "copying" ? (
        <DownloadProgress downloadedBytes={task.receivedBytes} totalBytes={task.totalBytes} />
      ) : null}
      {task?.phase === "done" ? (
        <span
          className="text-muted-foreground ml-2 text-xs"
          data-testid="media-download-done"
        >
          {t("chat.downloadSaved")}
        </span>
      ) : null}
      {task?.phase === "failed" ? (
        <span
          className="ml-2 text-xs break-all text-red-600 dark:text-red-300"
          data-testid="media-download-failed"
        >
          {t("chat.downloadFailed")}
          {task.error ? ": " + task.error : ""}
          <button
            type="button"
            data-testid="media-download-retry"
            className="ml-1 underline underline-offset-2"
            onClick={() => void pickAndExport()}
          >
            {t("chat.retry")}
          </button>
        </span>
      ) : null}
    </span>
  );
}
