import { useTranslation } from "react-i18next";

import type { Locale } from "@/i18n";
import { formatBytes } from "@/lib/format";

// 下载进度条：有 totalBytes 展示百分比与字节对，缺则不定进度（animate-pulse）。
export function DownloadProgress({
  downloadedBytes,
  totalBytes,
}: {
  downloadedBytes: number;
  totalBytes: number | null;
}) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const percent =
    totalBytes !== null && totalBytes > 0
      ? Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))
      : null;
  return (
    <div className="flex flex-col gap-1.5">
      <div
        className="bg-muted h-2 overflow-hidden rounded-full"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent ?? undefined}
      >
        <div
          className={
            percent === null
              ? "bg-primary h-full w-full animate-pulse"
              : "bg-primary h-full transition-all"
          }
          style={percent === null ? undefined : { width: percent + "%" }}
        />
      </div>
      <span className="text-muted-foreground text-xs">
        {totalBytes === null
          ? t("update.download.progressBytes", {
              downloaded: formatBytes(downloadedBytes, locale),
            })
          : t("update.download.progressPercent", {
              percent,
              downloaded: formatBytes(downloadedBytes, locale),
              total: formatBytes(totalBytes, locale),
            })}
      </span>
    </div>
  );
}
