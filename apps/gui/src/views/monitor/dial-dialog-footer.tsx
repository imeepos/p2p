import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import type { DialReport } from "@/lib/ipc-types";

interface DialDialogFooterProps {
  canSubmit: boolean;
  onClose: () => void;
  onSubmit: () => Promise<DialReport>;
  onCommandError: (message: string) => void;
}

// 拨号弹框页脚：关闭 + AsyncButton 提交；成功/失败 toast，错误回传内联展示。
export function DialDialogFooter({
  canSubmit,
  onClose,
  onSubmit,
  onCommandError,
}: DialDialogFooterProps) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
      <Button variant="outline" onClick={onClose}>
        {t("common.actions.close")}
      </Button>
      <AsyncButton
        disabled={!canSubmit}
        action={onSubmit}
        onSuccess={(result) => {
          const rep = result as DialReport;
          if (rep.ok) toastSuccess(t("peers.dial.succeeded"));
          else toastError(t("peers.dial.failed"));
        }}
        onError={(error) => {
          console.warn("[peers] 手动拨号失败", error);
          onCommandError(String(error));
          toastError(String(error));
        }}
      >
        {t("peers.dial.submit")}
      </AsyncButton>
    </div>
  );
}
