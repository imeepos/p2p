import {
  GlobeIcon,
  MoonIcon,
  SearchIcon,
  SunIcon,
  SunMoonIcon,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { requestOpenCommandPalette } from "@/components/command-palette/palette-bus";
import { NotificationBell } from "@/components/layout/notification-bell";
import { commandShortcutLabel } from "@/components/command-palette/shortcut";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { I18nKey } from "@/i18n/types";
import { changeLocale, SUPPORTED_LOCALES, type Locale } from "@/i18n";
import { useTheme, type Theme } from "@/theme/theme-provider";

function ThemeMenu() {
  const { t } = useTranslation();
  const { theme, setTheme } = useTheme();

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" aria-label={t("common.theme.label")}>
          {theme === "dark" ? (
            <MoonIcon />
          ) : theme === "light" ? (
            <SunIcon />
          ) : (
            <SunMoonIcon />
          )}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuRadioGroup
          value={theme}
          onValueChange={(value) => setTheme(value as Theme)}
        >
          <DropdownMenuRadioItem value="light">
            {t("common.theme.light")}
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="dark">
            {t("common.theme.dark")}
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="system">
            {t("common.theme.system")}
          </DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

const LOCALE_LABEL: Record<Locale, I18nKey> = {
  "zh-CN": "common.language.zhCN",
  "en-US": "common.language.enUS",
};

function LanguageMenu() {
  const { t, i18n } = useTranslation();
  const current = (SUPPORTED_LOCALES as readonly string[]).includes(
    i18n.language,
  )
    ? i18n.language
    : "zh-CN";

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          aria-label={t("common.language.label")}
        >
          <GlobeIcon />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuRadioGroup
          value={current}
          onValueChange={(value) => changeLocale(value as Locale)}
        >
          {SUPPORTED_LOCALES.map((locale) => (
            <DropdownMenuRadioItem key={locale} value={locale}>
              {t(LOCALE_LABEL[locale])}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

// 命令面板唯一可见入口（此前仅隐藏快捷键可发现）：徽标随平台显示 Cmd/Ctrl+K
function CommandPaletteButton() {
  const { t } = useTranslation();
  return (
    <Button
      variant="ghost"
      size="sm"
      className="text-muted-foreground h-6 gap-1.5 px-2"
      aria-label={t("palette.open")}
      title={t("palette.open")}
      onClick={requestOpenCommandPalette}
    >
      <SearchIcon className="size-3.5" />
      <kbd className="border-border bg-background text-muted-foreground pointer-events-none inline-flex h-4 items-center rounded border px-1 font-mono text-[10px] font-medium">
        {commandShortcutLabel()}
      </kbd>
    </Button>
  );
}

export function Topbar() {
  return (
    // macOS Overlay 标题栏（tauri.conf titleBarStyle）：顶栏即唯一标题栏，
    // 高度对齐系统红绿灯行（28px）；header 空白区可拖拽，右侧图标区保持可点击。
    // 标题文本与节点启停文字按钮已移除：状态看底部状态栏，启停走概览页状态卡。
    <header
      data-tauri-drag-region
      className="flex h-7 shrink-0 items-center justify-end gap-1 border-b px-3"
    >
      <div className="flex items-center gap-0.5 [&_button]:size-6 [&_[data-testid='notification-badge']]:-top-0.5 [&_[data-testid='notification-badge']]:-right-0.5">
        <NotificationBell />
        <CommandPaletteButton />
        <ThemeMenu />
        <LanguageMenu />
      </div>
    </header>
  );
}
