import { useEffect } from "react";
import { useNavigate } from "react-router-dom";

import { MENU_ENTRIES } from "@/config/menu.def";

function hasModifier(event: KeyboardEvent): boolean {
  return event.metaKey || event.ctrlKey;
}

/** Cmd/Ctrl+1..6 切换 menu.def 注册的前六个路由 */
export function useNumberRouteHotkeys() {
  const navigate = useNavigate();
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!hasModifier(event)) return;
      const index = Number(event.key) - 1;
      if (!Number.isInteger(index) || index < 0) return;
      if (index >= MENU_ENTRIES.length) return;
      event.preventDefault();
      navigate(MENU_ENTRIES[index].path);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [navigate]);
}

/** Cmd/Ctrl+K 触发命令面板 */
export function useCommandHotkey(onTrigger: () => void) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (hasModifier(event) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        onTrigger();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onTrigger]);
}

/** Esc 关闭浮层；enabled=false 时不监听 */
export function useEscapeKey(enabled: boolean, onEscape: () => void) {
  useEffect(() => {
    if (!enabled) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onEscape();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [enabled, onEscape]);
}
