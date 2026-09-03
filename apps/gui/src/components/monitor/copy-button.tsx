import { CopyIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Button, type ButtonProps } from "@/components/ui/button";
import { errorText } from "@/views/shared/form-flow";

type CopyButtonProps = { value: string } & Omit<
  ButtonProps,
  "onClick" | "children" | "asChild"
>;

// 剪贴板写入失败时 toast 报错并留 console 信号，不静默。
export function CopyButton({ value, ...props }: CopyButtonProps) {
  const { t } = useTranslation();

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      toastSuccess(t("common.copied"));
    } catch (error) {
      console.error("[clipboard] 写入失败", error);
      toastError(t("common.copyFailed"), { description: errorText(error) });
    }
  };

  return (
    <Button
      variant="ghost"
      size="icon"
      aria-label={t("common.actions.copy")}
      title={t("common.actions.copy")}
      onClick={() => void copy()}
      {...props}
    >
      <CopyIcon />
    </Button>
  );
}
