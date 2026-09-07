import {
  GlobeIcon,
  MoonIcon,
  SearchIcon,
  SunIcon,
  SunMoonIcon,
} from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { requestOpenCommandPalette } from "@/components/command-palette/palette-bus";
import { NotificationBell } from "@/components/layout/notification-bell";
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
import { StopNodeDialog } from "@/views/network/stop-node-dialog";
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

// F04：停止与概览状态卡同走 StopNodeDialog 二次确认，危险语义与文案一致；
// 启动非危险操作保持一步直达。停止失败/成功反馈由弹窗内 AsyncButton 承担。
function StartStopButton() {
  const { t } = useTranslation();
  const running = useNodeStore((s) => s.status?.running ?? false);
  const startNode = useNodeStore((s) => s.startNode);
  const [stopOpen, setStopOpen] = useState(false);

  const start = async () => {
    await startNode(await ipc.configGet());
  };

  return (
    <>
      {running ? (
        <Button
          size="sm"
          variant="outline"
          onClick={() => setStopOpen(true)}
          data-testid="topbar-stop-node"
        >
          {t("common.actions.stop")}
        </Button>
      ) : (
        <AsyncButton
          size="sm"
          variant="default"
          action={start}
          loadingLabel={t("common.state.starting")}
          onSuccess={() => toastSuccess(t("common.actions.startSucceeded"))}
          onError={(error) => {
            console.error("[topbar] 节点启动失败", error);
            toastError(t("common.actions.startFailed"), {
              description: errorText(error),
              context: "node.start",
            });
          }}
        >
          {t("common.actions.start")}
        </AsyncButton>
      )}
      <StopNodeDialog open={stopOpen} onOpenChange={setStopOpen} />
    </>
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
      <kbd className="border-border bg-background text-muted-foreground pointer-events-none inline-flex h-5 items-center rounded border px-1.5 font-mono text-[10px] font-medium">
        {commandShortcutLabel()}
      </kbd>
    </Button>
  );
}

export function Topbar() {
  const { t } = useTranslation();

  return (
    // macOS Overlay 标题栏（tauri.conf titleBarStyle）：顶栏即唯一标题栏，
    // header 与应用名带拖拽区属性；右侧按钮区不带属性保持可点击。
    <header
      data-tauri-drag-region
      className="flex h-14 shrink-0 items-center justify-between gap-3 border-b px-4"
    >
      <span
        data-tauri-drag-region
        className="text-sm font-semibold tracking-tight"
      >
        {t("common.appName")}
      </span>
      <div className="flex items-center gap-2">
        <NotificationBell />
        <CommandPaletteButton />
        <NodeStatusPill />
        <StartStopButton />
        <ThemeMenu />
        <LanguageMenu />
      </div>
    </header>
  );
}
