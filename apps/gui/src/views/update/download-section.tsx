import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { useUpdateStore } from "@/stores/update-store";

import { DownloadProgress } from "./download-progress";
import { openReleasePage } from "./release-links";

// 契约 v8 §13：下载安装操作段，设置卡与提醒对话框共用。
// 相位驱动：idle=按钮 → downloading=进度条 → installed=重启入口；failed 可重试；
// 浏览器打开始终保留为逃生通道（签名/网络环境异常时仍可手动下载）。
export function DownloadSection({ size = "default" }: { size?: "sm" | "default" }) {
  const { t } = useTranslation();
  const phase = useUpdateStore((s) => s.downloadPhase);
  const downloadedBytes = useUpdateStore((s) => s.downloadedBytes);
  const totalBytes = useUpdateStore((s) => s.totalBytes);
  const downloadError = useUpdateStore((s) => s.downloadError);
  const downloadAndInstall = useUpdateStore((s) => s.downloadAndInstall);
  const relaunch = useUpdateStore((s) => s.relaunch);
  const releaseUrl = useUpdateStore((s) => s.result?.releaseUrl ?? null);

  const fallback = (
    <Button
      type="button"
      size={size}
      variant="outline"
      onClick={() => void openReleasePage(releaseUrl)}
    >
      {t("update.detail.openInBrowser")}
    </Button>
  );

  if (phase === "downloading") {
    return (
      <div className="flex flex-col gap-3">
        <DownloadProgress downloadedBytes={downloadedBytes} totalBytes={totalBytes} />
        {fallback}
      </div>
    );
  }
  if (phase === "installed") {
    return (
      <div className="flex flex-col gap-2">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm">{t("update.download.installed")}</span>
          <Button type="button" size={size} onClick={relaunch}>
            {t("update.download.restart")}
          </Button>
        </div>
        {fallback}
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          size={size}
          onClick={() => void downloadAndInstall()}
        >
          {phase === "failed" ? t("update.download.retry") : t("update.download.button")}
        </Button>
        {fallback}
      </div>
      {phase === "failed" ? (
        <span className="text-destructive text-sm">
          {t("update.download.failed")}: {downloadError}
        </span>
      ) : null}
    </div>
  );
}
