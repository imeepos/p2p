import { useEffect } from "react";
import { useNavigate } from "react-router-dom";

import { MENU_ENTRIES } from "@/config/menu.def";

/** 数字路由热键只认 1..9：0 不跳页，第 10 个起的注册路由无数字热键 */
const MAX_NUMBER_HOTKEY = 9;

const FOCUS_DIALOG_SELECTOR = '[role="dialog"], [role="alertdialog"]';
const OPEN_DIALOG_SELECTOR =
  '[role="dialog"][data-state="open"], [role="alertdialog"][data-state="open"]';

function hasModifier(event: KeyboardEvent): boolean {
  return event.metaKey || event.ctrlKey;
}

// 对话框打开或焦点落在对话框内时退避：用户半填的表单不能因路由切换随组件卸载静默丢失
function isDialogActive(event: KeyboardEvent): boolean {
  const target = event.target;
  if (target instanceof Element && target.closest(FOCUS_DIALOG_SELECTOR)) {
    return true;
  }
  return document.querySelector(OPEN_DIALOG_SELECTOR) !== null;
}

/** Cmd/Ctrl+1..9 切换 menu.def 注册的前九个路由 */
export function useNumberRouteHotkeys() {
  const navigate = useNavigate();
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!hasModifier(event)) return;
      const digit = Number(event.key);
      if (!Number.isInteger(digit) || digit < 1) return;
      if (digit > MAX_NUMBER_HOTKEY) return;
      const index = digit - 1;
      if (index >= MENU_ENTRIES.length) return;
      if (isDialogActive(event)) return;
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
