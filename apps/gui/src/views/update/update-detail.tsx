import type { ReactNode } from "react";

import { useTranslation } from "react-i18next";

import type { Locale } from "@/i18n";
import { formatDateTime } from "@/lib/format";
import type { UpdateCheckResult } from "@/lib/ipc-types";

import { MAX_NOTES_CHARS, truncateNotes } from "./release-links";

function DetailRow({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-muted-foreground text-xs">{label}</span>
      {children}
    </div>
  );
}

// 更新详情块：版本（可选）/发布名/说明（超长截断）/发布时间。
export function UpdateDetail({
  result,
  showVersion = false,
}: {
  result: UpdateCheckResult;
  showVersion?: boolean;
}) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const notes = result.releaseNotesMd;
  const truncated = notes !== null && notes.length > MAX_NOTES_CHARS;

  return (
    <div className="text-sm flex flex-col gap-3">
      {showVersion && result.latestVersion ? (
        <DetailRow label={t("common.labels.version")}>
          <span>{result.latestVersion}</span>
        </DetailRow>
      ) : null}
      {result.releaseName ? (
        <DetailRow label={t("update.detail.releaseName")}>
          <span>{result.releaseName}</span>
        </DetailRow>
      ) : null}
      {notes ? (
        <DetailRow label={t("update.detail.notes")}>
          <p className="bg-muted/40 border rounded-md p-3 break-words whitespace-pre-wrap">
            {truncateNotes(notes)}
          </p>
          {truncated ? (
            <span className="text-muted-foreground text-xs">
              {t("update.detail.notesTruncated")}
            </span>
          ) : null}
        </DetailRow>
      ) : null}
      {result.publishedAtMs !== null ? (
        <DetailRow label={t("update.detail.publishedAt")}>
          <span>{formatDateTime(result.publishedAtMs, locale)}</span>
        </DetailRow>
      ) : null}
    </div>
  );
}
