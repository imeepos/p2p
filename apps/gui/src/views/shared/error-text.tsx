import { useTranslation } from "react-i18next";

import type { I18nKey } from "@/i18n/types";

// 校验错误统一渲染为内联红字；message 是稳定代码，经 i18n 出双语。
export function ErrorText({ code }: { code?: string }) {
  const { t } = useTranslation();
  if (!code) return null;
  return (
    <p className="text-destructive text-xs">
      {t(`common.validation.${code}` as I18nKey)}
    </p>
  );
}
