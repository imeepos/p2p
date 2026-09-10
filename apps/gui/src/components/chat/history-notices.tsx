import { AlertCircle } from "lucide-react";
import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";

// 历史加载失败错误态（IM-T50）：可读文案 + 原始错误详情 + 重试入口；
// 失败不白屏。普通路径与虚拟化路径共用，保证两种渲染模式信号一致。
export function HistoryErrorNotice({
  detail,
  onRetry,
}: {
  detail: string;
  onRetry: () => Promise<unknown>;
}) {
  const { t } = useTranslation();
  return (
    <div
      data-testid="chat-history-error"
      className="flex flex-col items-center gap-1.5 text-center"
    >
      <p className="flex items-center gap-1.5 text-sm font-medium text-destructive">
        <AlertCircle aria-hidden className="size-4" />
        {t("chat.historyLoadFailed")}
      </p>
      <p className="max-w-80 text-xs break-all text-muted-foreground">{detail}</p>
      <AsyncButton
        type="button"
        size="sm"
        variant="outline"
        className="mt-1"
        action={onRetry}
        onError={(error) => console.error("[chat] 历史加载重试失败", error)}
      >
        {t("chat.retry")}
      </AsyncButton>
    </div>
  );
}

// 更早分页失败信号（IM-T50）：顶部横幅 + 重试，禁止静默。
export function OlderErrorBanner({
  detail,
  onRetry,
}: {
  detail: string;
  onRetry: () => Promise<unknown>;
}) {
  const { t } = useTranslation();
  return (
    <div
      data-testid="chat-older-error"
      className="flex items-center justify-center gap-2 py-2 text-xs"
    >
      <span className="text-destructive">{t("chat.loadOlderFailed")}</span>
      <span className="max-w-64 truncate text-muted-foreground">{detail}</span>
      <AsyncButton
        type="button"
        size="sm"
        variant="outline"
        action={onRetry}
        onError={(error) => console.error("[chat] 更早历史重试失败", error)}
      >
        {t("chat.retry")}
      </AsyncButton>
    </div>
  );
}

// 加载更早进行中的顶部提示。
export function LoadingHistoryHint() {
  const { t } = useTranslation();
  return (
    <p className="py-2 text-center text-xs text-muted-foreground">
      {t("chat.loadingHistory")}
    </p>
  );
}
