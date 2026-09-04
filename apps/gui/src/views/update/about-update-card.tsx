import { useCallback } from "react";
import { useTranslation } from "react-i18next";

import { toastError } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import type { Locale } from "@/i18n";
import { formatDateTime } from "@/lib/format";
import { useUpdateStore } from "@/stores/update-store";

import { DownloadSection } from "./download-section";
import { UpdateDetail } from "./update-detail";

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

// 设置"关于与更新"卡：当前版本 + 手动检查三态 + 有更新详情与跳过入口。
export function AboutUpdateCard() {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const status = useUpdateStore((s) => s.status);
  const result = useUpdateStore((s) => s.result);
  const checkedAtMs = useUpdateStore((s) => s.result?.checkedAtMs ?? null);
  const skippedVersion = useUpdateStore((s) => s.skippedVersion);
  const skipCurrentVersion = useUpdateStore((s) => s.skipCurrentVersion);
  const downloadPhase = useUpdateStore((s) => s.downloadPhase);
  const checkNow = useManualCheck();

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("settings.cards.about")}</CardTitle>
        <CardDescription>{t("update.status.autoHint")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="flex items-center justify-between gap-2">
          <span className="text-muted-foreground text-sm">
            {t("update.about.currentVersion")}
          </span>
          <span className="text-sm font-medium">v{__APP_VERSION__}</span>
        </div>
        <div className="flex flex-wrap items-center gap-2">
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
          <CheckStateLine />
        </div>
        {checkedAtMs !== null && status !== "checking" ? (
          <span className="text-muted-foreground text-xs">
            {t("update.status.checkedAt", {
              time: formatDateTime(checkedAtMs, locale),
            })}
          </span>
        ) : null}
        {skippedVersion ? (
          <span className="text-muted-foreground text-xs">
            {t("update.about.skipped", { version: skippedVersion })}
          </span>
        ) : null}
        {status === "available" && result ? (
          <div className="flex flex-col gap-3">
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
      </CardContent>
    </Card>
  );
}
