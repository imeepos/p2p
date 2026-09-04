import { useTranslation } from "react-i18next";

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { changeLocale, SUPPORTED_LOCALES, type Locale } from "@/i18n";
import type { I18nKey } from "@/i18n/types";
import { useTheme, type Theme } from "@/theme/theme-provider";

const THEME_OPTIONS: Theme[] = ["light", "dark", "system"];

function OptionButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: string;
}) {
  // 未选中态加强描边（IM-V2 S2）：gray-300 深一档，浅色底上边界可辨。
  return (
    <Button
      type="button"
      size="sm"
      variant={active ? "default" : "outline"}
      aria-pressed={active}
      className={
        active ? undefined : "border-gray-300 bg-background dark:border-gray-600"
      }
      onClick={onClick}
    >
      {children}
    </Button>
  );
}

// 外观卡：复用一等能力（主题三选一 + 语言切换），不属于节点配置表单。
export function AppearanceCard() {
  const { t, i18n } = useTranslation();
  const { theme, setTheme } = useTheme();

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("settings.cards.appearance")}</CardTitle>
        <CardDescription>{t("settings.appearance.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="flex flex-col gap-1.5">
          <Label>{t("common.theme.label")}</Label>
          <div className="flex gap-2">
            {THEME_OPTIONS.map((option) => (
              <OptionButton
                key={option}
                active={theme === option}
                onClick={() => setTheme(option)}
              >
                {t(`common.theme.${option}` as I18nKey)}
              </OptionButton>
            ))}
          </div>
        </div>
        <div className="flex flex-col gap-1.5">
          <Label>{t("common.language.label")}</Label>
          <div className="flex gap-2">
            {SUPPORTED_LOCALES.map((locale) => (
              <OptionButton
                key={locale}
                active={i18n.language === locale}
                onClick={() => changeLocale(locale as Locale)}
              >
                {t(
                  (locale === "zh-CN"
                    ? "common.language.zhCN"
                    : "common.language.enUS") as I18nKey,
                )}
              </OptionButton>
            ))}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
