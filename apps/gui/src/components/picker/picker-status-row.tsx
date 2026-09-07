import { useTranslation } from "react-i18next";
import { Loader2Icon } from "lucide-react";

import { Button } from "@/components/ui/button";

interface PickerStatusRowProps {
  loading?: boolean;
  error?: string | null;
  onRetry?: () => void;
}

// 选择器列表区三态（加载/错误/空）共享行：单选下拉与多选内嵌列表共用，
// 保证全站选择器数据面表现一致。空态文案由调用方按列表是否真无数据决定
// 渲染时机（本组件只管加载与错误两态）。
export function PickerStatusRow({ loading, error, onRetry }: PickerStatusRowProps) {
  const { t } = useTranslation();
  if (loading) {
    return (
      <p
        className="text-muted-foreground flex items-center gap-2 px-2 py-1.5 text-sm"
        data-testid="picker-loading"
        role="status"
      >
        <Loader2Icon aria-hidden className="size-4 animate-spin" />
        {t("picker.loading")}
      </p>
    );
  }
  if (error) {
    return (
      <div
        className="flex flex-col gap-1 px-2 py-1.5"
        data-testid="picker-error"
        role="alert"
      >
        <p className="text-destructive text-sm">{t("picker.loadFailed")}</p>
        {onRetry ? (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="w-fit"
            onClick={onRetry}
            data-testid="picker-retry"
          >
            {t("picker.retry")}
          </Button>
        ) : null}
      </div>
    );
  }
  return (
    <p className="text-muted-foreground px-2 py-1.5 text-sm" data-testid="picker-empty">
      {t("picker.empty")}
    </p>
  );
}
