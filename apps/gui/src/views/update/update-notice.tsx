import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useUpdateStore } from "@/stores/update-store";

import { UpdateDetail } from "./update-detail";
import { openReleasePage } from "./release-links";

const REMINDER_TOAST_DURATION_MS = 8000;

// 提醒判定：hasUpdate 且未跳过该版本且该版本尚未提醒过。
// toast 去重读 getState 而非闭包值：StrictMode 双执行 effect 也只弹一次。
function useUpdateReminderToast(): [boolean, (open: boolean) => void] {
  const { t } = useTranslation();
  const result = useUpdateStore((s) => s.result);
  const skippedVersion = useUpdateStore((s) => s.skippedVersion);
  const markReminderShown = useUpdateStore((s) => s.markReminderShown);
  const [detailOpen, setDetailOpen] = useState(false);

  const latestVersion = result?.latestVersion ?? null;
  const shouldRemind =
    result !== null &&
    result.hasUpdate &&
    latestVersion !== null &&
    latestVersion !== skippedVersion;

  useEffect(() => {
    if (!shouldRemind || !latestVersion) return;
    const state = useUpdateStore.getState();
    if (state.reminderShownFor === latestVersion) return;
    state.markReminderShown(latestVersion);
    toast.message(t("update.reminder.title", { version: latestVersion }), {
      description: result?.releaseName ?? undefined,
      duration: REMINDER_TOAST_DURATION_MS,
      action: {
        label: t("update.reminder.action"),
        onClick: () => setDetailOpen(true),
      },
    });
  }, [shouldRemind, latestVersion, result, markReminderShown, t]);

  return [detailOpen, setDetailOpen];
}

// 全局挂载（AppLayout）：有更新时弹非侵入 toast，"查看详情"开对话框；
// 对话框内"前往下载/阅读完整说明"均经 update_open_release_page。
export function UpdateNotice() {
  const [detailOpen, setDetailOpen] = useUpdateReminderToast();
  const { t } = useTranslation();
  const result = useUpdateStore((s) => s.result);
  const releaseUrl = result?.releaseUrl ?? null;

  return (
    <Dialog open={detailOpen} onOpenChange={setDetailOpen}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {t("update.detail.dialogTitle", {
              version: result?.latestVersion ?? "",
            })}
          </DialogTitle>
          {result?.releaseName ? (
            <DialogDescription>{result.releaseName}</DialogDescription>
          ) : null}
        </DialogHeader>
        {result ? <UpdateDetail result={result} showVersion /> : null}
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => void openReleasePage(releaseUrl)}
          >
            {t("update.detail.openInBrowser")}
          </Button>
          <Button
            type="button"
            onClick={() => void openReleasePage(releaseUrl)}
          >
            {t("update.detail.download")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
