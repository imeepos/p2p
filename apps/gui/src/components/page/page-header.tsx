import { useTranslation } from "react-i18next";

import type { I18nKey } from "@/i18n/types";

interface PageHeaderProps {
  titleKey: I18nKey;
  descriptionKey: I18nKey;
}

export function PageHeader({ titleKey, descriptionKey }: PageHeaderProps) {
  const { t } = useTranslation();

  return (
    <div className="col-span-12 flex flex-col gap-1">
      <h1 className="text-xl font-semibold tracking-tight">{t(titleKey)}</h1>
      <p className="text-muted-foreground text-sm">{t(descriptionKey)}</p>
    </div>
  );
}
