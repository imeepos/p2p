import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Outlet, useLocation } from "react-router-dom";

import { CommandPalette } from "@/components/command-palette/command-palette";
import { Sidebar } from "@/components/layout/sidebar";
import { StatusBar } from "@/components/layout/status-bar";
import { Topbar } from "@/components/layout/topbar";
import {
  useCommandHotkey,
  useNumberRouteHotkeys,
} from "@/hooks/use-hotkeys";
import { useNodeStore } from "@/stores/node-store";
import { useUpdateStore } from "@/stores/update-store";
import { UpdateNotice } from "@/views/update/update-notice";

const REFRESH_INTERVAL_MS = 5000;

export function AppLayout() {
  const { t } = useTranslation();
  const location = useLocation();
  const [collapsed, setCollapsed] = useState(false);
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

  return (
    <div className="flex h-screen w-full overflow-hidden">
      <Sidebar collapsed={collapsed} onToggle={() => setCollapsed((v) => !v)} />
      <div className="flex min-w-0 flex-1 flex-col">
        <Topbar />
        <main className="flex-1 overflow-y-auto">
          <div
            className="grid grid-cols-12 gap-4 p-6"
            aria-label={t("common.appName")}
          >
            <div
              key={location.pathname}
              className="motion-safe:animate-in motion-safe:fade-in motion-safe:duration-300 col-span-12 grid grid-cols-12 gap-4"
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
