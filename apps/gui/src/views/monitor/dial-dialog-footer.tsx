import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import type { DialReport } from "@/lib/ipc-types";
import { errorText, isFlowMark, FORM_VALIDATION_MARK } from "@/views/shared/form-flow";

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
        loadingLabel={t("common.state.dialing")}
        onSuccess={(result) => {
          const rep = result as DialReport;
          if (rep.ok) {
            toastSuccess(t("peers.dial.succeeded"));
            return;
          }
          const failReason =
            rep.hops.find((hop) => !hop.ok)?.detail ??
            t("peers.dial.failedReasonUnknown");
          toastError(t("peers.dial.failed"), {
            description: failReason,
            context: "peer.dial",
          });
        }}
        onError={(error) => {
          // 业务校验类中断（如节点未运行）已内联展示，不再重复 toast。
          if (isFlowMark(error, FORM_VALIDATION_MARK)) return;
          console.error("[peers] 手动拨号命令失败", error);
          onCommandError(errorText(error));
          toastError(t("peers.dial.failed"), {
            description: errorText(error),
            context: "peer.dial",
          });
        }}
      >
        {t("peers.dial.submit")}
      </AsyncButton>
    </div>
  );
}
