import { useCallback } from "react";
import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { errorText } from "@/views/shared/form-flow";

interface StartNodeButtonProps {
  action: () => Promise<unknown>;
  disabled?: boolean;
}

// 启动节点按钮（monitor 域共用）：成功/失败 toast，失败留在原位可重试。
export function StartNodeButton({ action, disabled }: StartNodeButtonProps) {
  const { t } = useTranslation();
  const onStart = useCallback(() => action(), [action]);

  return (
    <AsyncButton
      type="button"
      size="sm"
      disabled={disabled}
      action={onStart}
      loadingLabel={t("common.state.starting")}
      onSuccess={() => toastSuccess(t("common.actions.startSucceeded"))}
      onError={(error) => {
        console.error("[monitor] 启动节点失败", error);
        toastError(t("common.actions.startFailed"), {
          description: errorText(error),
          context: "node.start",
        });
      }}
    >
      {t("common.actions.start")}
    </AsyncButton>
  );
}
