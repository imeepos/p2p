import { GlobeIcon, MoonIcon, SunIcon, SunMoonIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Badge } from "@/components/ui/badge";
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
import { cn } from "@/lib/utils";
import { ipc } from "@/lib/ipc";
import { useNodeStore } from "@/stores/node-store";
import { useTheme, type Theme } from "@/theme/theme-provider";

function NodeStatusPill() {
  const { t } = useTranslation();
  const running = useNodeStore((s) => s.status?.running ?? false);
  const peerId = useNodeStore((s) => s.status?.peerId ?? null);

  return (
    <Badge variant={running ? "default" : "outline"} className="gap-1.5">
      <span
        className={cn(
          "size-1.5 rounded-full",
          running
            ? "motion-safe:animate-pulse bg-success"
            : "bg-muted-foreground",
        )}
        aria-hidden
      />
      {running ? t("common.state.running") : t("common.state.stopped")}
      {peerId && (
        <span className="hidden font-mono text-xs opacity-70 md:inline">
          {peerId.slice(0, 8)}
        </span>
      )}
    </Badge>
  );
}

function StartStopButton() {
  const { t } = useTranslation();
  const running = useNodeStore((s) => s.status?.running ?? false);
  const startNode = useNodeStore((s) => s.startNode);
  const stopNode = useNodeStore((s) => s.stopNode);

  const action = async () => {
    if (running) {
      await stopNode();
    } else {
      await startNode(await ipc.configGet());
    }
  };

  return (
    <AsyncButton
      size="sm"
      variant={running ? "destructive" : "default"}
      action={action}
      onSuccess={() =>
        toastSuccess(
          t(
            running
              ? "common.actions.stopSucceeded"
              : "common.actions.startSucceeded",
          ),
        )
      }
      onError={(error) => toastError(String(error))}
    >
      {running ? t("common.actions.stop") : t("common.actions.start")}
    </AsyncButton>
  );
}

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

export function Topbar() {
  const { t } = useTranslation();

  return (
    <header className="flex h-14 shrink-0 items-center justify-between gap-3 border-b px-4">
      <span className="text-sm font-semibold tracking-tight">
        {t("common.appName")}
      </span>
      <div className="flex items-center gap-2">
        <NodeStatusPill />
        <StartStopButton />
        <ThemeMenu />
        <LanguageMenu />
      </div>
    </header>
  );
}
