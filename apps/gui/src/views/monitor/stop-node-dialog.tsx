import { useTranslation } from "react-i18next";
import { useCallback } from "react";

import { AsyncButton } from "@/components/feedback/async-button";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { useNodeStore } from "@/stores/node-store";

interface StopNodeDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

// 停止节点：AlertDialog 二次确认 + 影响说明；AsyncButton 提交，失败留在弹框可重试。
export function StopNodeDialog({ open, onOpenChange }: StopNodeDialogProps) {
  const { t } = useTranslation();
  const stopNode = useNodeStore((s) => s.stopNode);

  const onStop = useCallback(async () => {
    await stopNode();
  }, [stopNode]);

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            {t("dashboard.stopConfirm.title")}
          </AlertDialogTitle>
          <AlertDialogDescription>
            {t("dashboard.stopConfirm.description")}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{t("common.actions.cancel")}</AlertDialogCancel>
          <AsyncButton
            variant="destructive"
            action={onStop}
            onSuccess={() => {
              onOpenChange(false);
              toastSuccess(t("common.actions.stopSucceeded"));
            }}
            onError={(error) => {
              console.error("[dashboard] 停止节点失败", error);
              toastError(String(error));
            }}
          >
            {t("common.actions.confirm")}
          </AsyncButton>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
