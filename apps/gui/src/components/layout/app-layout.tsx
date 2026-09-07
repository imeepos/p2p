import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Outlet, useLocation } from "react-router-dom";

import { CommandPalette } from "@/components/command-palette/command-palette";
import { IconRail } from "@/components/layout/icon-rail";
import { StatusBar } from "@/components/layout/status-bar";
import {
  useCommandHotkey,
  useNumberRouteHotkeys,
} from "@/hooks/use-hotkeys";
import { useNodeAutoStart } from "@/hooks/use-node-auto-start";
import { useNodeStore } from "@/stores/node-store";
import { useUpdateStore } from "@/stores/update-store";
import { cn } from "@/lib/utils";
import { DataLinkBanner } from "@/views/network/data-link-banner";
import { AutoStartNotice } from "@/views/shared/auto-start-notice";
import { UpdateNotice } from "@/views/update/update-notice";

const REFRESH_INTERVAL_MS = 5000;

export function AppLayout() {
  const { t } = useTranslation();
  const location = useLocation();
  const [paletteOpen, setPaletteOpen] = useState(false);
  const bootstrap = useNodeStore((s) => s.bootstrap);
  const refresh = useNodeStore((s) => s.refresh);
  const startAutoCheck = useUpdateStore((s) => s.startAutoCheck);
  const stopAutoCheck = useUpdateStore((s) => s.stopAutoCheck);
  const openPalette = useCallback(() => setPaletteOpen(true), []);

  useNumberRouteHotkeys();
  useCommandHotkey(openPalette);

  useEffect(() => {
    void bootstrap();
  }, [bootstrap]);

  // UX1 启动即在线：引导就绪且节点未运行时自动启动一次（闸门在 store 内）。
  useNodeAutoStart();

  // 更新自动检查：启动即查一次 + 每 4h 轮询（定时器在 store 模块层，幂等启停）
  useEffect(() => {
    startAutoCheck();
    return () => stopAutoCheck();
  }, [startAutoCheck, stopAutoCheck]);

  useEffect(() => {
    const timer = window.setInterval(() => {
      void refresh();
    }, REFRESH_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [refresh]);

  // WX1：聊天页全出血（会话列表/记录贴边铺满），其余页面维持卡片栅格
  const fullBleed = location.pathname.startsWith("/chat");

  return (
    <div className="flex h-dvh w-full overflow-hidden">
      <IconRail />
      <div className="flex min-w-0 flex-1 flex-col">
        <main className="flex min-h-0 flex-1 flex-col overflow-y-auto">
          <DataLinkBanner />
          <AutoStartNotice />
          <div
            className={cn(
              "grid min-h-0 flex-1 grid-cols-12",
              fullBleed ? "overflow-hidden" : "gap-4 p-6",
            )}
            aria-label={t("common.appName")}
          >
            <div
              key={location.pathname}
              className={cn(
                "motion-safe:animate-in motion-safe:fade-in motion-safe:duration-300 col-span-12 flex min-h-0 flex-col",
                fullBleed ? "" : "gap-4",
              )}
            >
              <Outlet />
            </div>
          </div>
        </main>
        <StatusBar />
      </div>
      <CommandPalette open={paletteOpen} onOpenChange={setPaletteOpen} />
      <UpdateNotice />
    </div>
  );
}
