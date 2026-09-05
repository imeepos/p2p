import {
  GlobeIcon,
  MoonIcon,
  SearchIcon,
  SunIcon,
  SunMoonIcon,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { requestOpenCommandPalette } from "@/components/command-palette/palette-bus";
import { commandShortcutLabel } from "@/components/command-palette/shortcut";
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
import { errorText } from "@/views/shared/form-flow";
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

  // 运行中停止操作走中性边框（IM-V2 D1）：红色 destructive 只保留在
  // 二次确认弹框内，页面常驻按钮不再出现红色系。
  return (
    <AsyncButton
      size="sm"
      variant={running ? "outline" : "default"}
      action={action}
      loadingLabel={
        running ? t("common.state.stopping") : t("common.state.starting")
      }
      onSuccess={() =>
        toastSuccess(
          t(
            running
              ? "common.actions.stopSucceeded"
              : "common.actions.startSucceeded",
          ),
        )
      }
      onError={(error) => {
        console.error("[topbar] 节点启停失败", error);
        toastError(
          running
            ? t("common.actions.stopFailed")
            : t("common.actions.startFailed"),
          {
            description: errorText(error),
            context: running ? "node.stop" : "node.start",
          },
        );
      }}
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

// 命令面板唯一可见入口（此前仅隐藏快捷键可发现）：徽标随平台显示 Cmd/Ctrl+K
function CommandPaletteButton() {
  const { t } = useTranslation();
  return (
    <Button
      variant="ghost"
      size="sm"
      className="text-muted-foreground gap-1.5 px-2"
      aria-label={t("palette.open")}
      title={t("palette.open")}
      onClick={requestOpenCommandPalette}
    >
      <SearchIcon className="size-4" />
      <kbd className="border-border bg-muted text-muted-foreground pointer-events-none inline-flex h-5 items-center rounded border px-1.5 font-mono text-[10px] font-medium">
        {commandShortcutLabel()}
      </kbd>
    </Button>
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
        <CommandPaletteButton />
        <NodeStatusPill />
        <StartStopButton />
        <ThemeMenu />
        <LanguageMenu />
      </div>
    </header>
  );
}
