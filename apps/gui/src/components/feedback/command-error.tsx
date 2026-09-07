import { useTranslation } from "react-i18next";

import { CopyButton } from "@/components/feedback/copy-button";
import type { I18nKey } from "@/i18n/types";

interface CommandErrorTextProps {
  /** 后端拒绝原文：不翻译不吞，与 toastError 的复制链路对齐（C1） */
  message: string | null;
  /** 失败语境前缀（如「添加失败：」），可选 */
  prefixKey?: I18nKey;
  testId?: string;
}

// 对话框/表单内联失败原因的标准呈现：红字 role=alert + 原文 + 复制详情。
// 全站行内错误统一走此件，失败原因不再只有裸 <p> 原文不可复制。
export function CommandErrorText({ message, prefixKey, testId }: CommandErrorTextProps) {
  const { t } = useTranslation();
  if (!message) return null;
  return (
    <p
      className="text-destructive flex items-start justify-between gap-2 text-xs"
      role="alert"
      data-testid={testId}
    >
      <span className="min-w-0 flex-1 break-all">
        {prefixKey ? t(prefixKey) : null}
        {message}
      </span>
      <CopyButton value={message} className="size-5 shrink-0" />
    </p>
  );
}
