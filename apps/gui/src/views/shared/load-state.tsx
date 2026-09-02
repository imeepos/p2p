import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { toastError } from "@/components/feedback/toast";
import type { I18nKey } from "@/i18n/types";

interface LoadFailedNoticeProps {
  onRetry: () => Promise<void>;
  messageKey: I18nKey;
}

// 配置加载失败态：红字提示 + 重试按钮，失败路径 toast + console 双通道。
export function LoadFailedNotice({ onRetry, messageKey }: LoadFailedNoticeProps) {
  const { t } = useTranslation();

  return (
    <div className="col-span-12 flex flex-col items-start gap-2">
      <p className="text-destructive text-sm">{t(messageKey)}</p>
      <AsyncButton
        type="button"
        size="sm"
        variant="outline"
        action={onRetry}
        onError={(error) => {
          console.error("[views] config_get 重试失败", error);
          toastError(t(messageKey));
        }}
      >
        {t("common.actions.refresh")}
      </AsyncButton>
    </div>
  );
}
