import { useCallback } from "react";
import { useTranslation } from "react-i18next";

import { toastError } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import type { Locale } from "@/i18n";
import { formatDateTime } from "@/lib/format";
import { useUpdateStore } from "@/stores/update-store";

import { DownloadSection } from "./download-section";
import { UpdateDetail } from "./update-detail";
import { SettingsGroup, SettingsRow } from "@/views/settings/settings-row";

// 手动检查：失败必须可见可重试（toast + 行内失败态）；自动检查失败仅落状态。
function useManualCheck() {
  const { t } = useTranslation();
  const check = useUpdateStore((s) => s.check);
  return useCallback(async () => {
    await check("manual");
    const state = useUpdateStore.getState();
    if (state.status === "failed") {
      toastError(t("update.status.failed"), {
        description: state.error ?? undefined,
      });
    }
  }, [check, t]);
}

function CheckStateLine() {
  const { t } = useTranslation();
  const status = useUpdateStore((s) => s.status);
  const error = useUpdateStore((s) => s.error);
  const checkNow = useManualCheck();

  if (status === "upToDate") {
    return (
      <span className="text-muted-foreground text-sm">
        {t("update.status.upToDate")}
      </span>
    );
  }
  if (status === "failed") {
    return (
      <>
        <span className="text-destructive text-sm">
          {t("update.status.failed")}: {error}
        </span>
        <Button
          type="button"
          size="sm"
          variant="outline"
          onClick={() => void checkNow()}
        >
          {t("update.status.retry")}
        </Button>
      </>
    );
  }
  return null;
}

// 设置「关于与更新」组：版本行 + 手动检查三态 + 有更新详情与跳过入口。
export function AboutUpdateCard() {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const status = useUpdateStore((s) => s.status);
  const result = useUpdateStore((s) => s.result);
  const checkedAtMs = useUpdateStore((s) => s.result?.checkedAtMs ?? null);
  const skippedVersion = useUpdateStore((s) => s.skippedVersion);
  const skipCurrentVersion = useUpdateStore((s) => s.skipCurrentVersion);
  const unskipVersion = useUpdateStore((s) => s.unskipVersion);
  const downloadPhase = useUpdateStore((s) => s.downloadPhase);
  const checkNow = useManualCheck();

  return (
    <SettingsGroup
      title={t("settings.cards.about")}
      description={t("update.status.autoHint")}
    >
      <SettingsRow
        label={t("update.about.currentVersion")}
        control={
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium">v{__APP_VERSION__}</span>
            <Button
              type="button"
              size="sm"
              disabled={status === "checking"}
              onClick={() => void checkNow()}
            >
              {status === "checking"
                ? t("update.status.checking")
                : t("update.about.checkNow")}
            </Button>
          </div>
        }
      />
      {status === "upToDate" || status === "failed" ? (
        <SettingsRow control={<CheckStateLine />} />
      ) : null}
      {checkedAtMs !== null && status !== "checking" ? (
        <div className="py-1">
          <span className="text-muted-foreground text-xs">
            {t("update.status.checkedAt", {
              time: formatDateTime(checkedAtMs, locale),
            })}
          </span>
        </div>
      ) : null}
      {skippedVersion ? (
        <div className="text-muted-foreground flex items-center gap-2 py-1 text-xs">
          {t("update.about.skipped", { version: skippedVersion })}
          <Button
            type="button"
            size="sm"
            variant="outline"
            data-testid="update-unskip"
            onClick={unskipVersion}
          >
            {t("update.about.unskip")}
          </Button>
        </div>
      ) : null}
      {status === "available" && result ? (
        <div className="flex flex-col gap-3 pt-2">
          <Separator />
          <span className="text-sm font-medium">
            {t("update.reminder.title", {
              version: result.latestVersion ?? "",
            })}
          </span>
          <UpdateDetail result={result} />
          {downloadPhase === "idle" ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={skipCurrentVersion}
            >
              {t("update.about.skip")}
            </Button>
          ) : null}
          <DownloadSection size="sm" />
        </div>
      ) : null}
    </SettingsGroup>
  );
}
