import { useCallback, useEffect, useState } from "react";

import { registerReloader, startDataWatch } from "@/lib/data-watch";
import { ipc } from "@/lib/ipc";
import type { GuiConfig } from "@/lib/ipc-types";

// 发现/中继视图共用的持久化配置读取：null = 加载中，failed = 加载失败可重试。
// IPC config_get 是外部系统，挂载拉取属 effect 的合法同步场景。
export function useGuiConfig() {
  const [config, setConfig] = useState<GuiConfig | null>(null);
  const [failed, setFailed] = useState(false);

  const reload = useCallback(async () => {
    setConfig(await ipc.configGet());
    setFailed(false);
  }, []);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const cfg = await ipc.configGet();
        if (!cancelled) setConfig(cfg);
      } catch (error) {
        console.error("[config] config_get 失败", error);
        if (!cancelled) setFailed(true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [reload]);

  // W1：CLI 改写 gui-config.json → data-changed[config] → 挂载中的视图定向重载。
  // startDataWatch 幂等单例；注销避免卸载视图被无主重载。
  useEffect(() => {
    void startDataWatch();
    return registerReloader("config", () => void reload());
  }, [reload]);

  return { config, failed, reload };
}
