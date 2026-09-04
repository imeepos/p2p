// GC1 控制通道前端桥：把当前 hash 路由实时上报给 Rust（health 的 route 字段来源）。
// navigate 命令由 Rust 侧直接 eval location.hash（HashRouter 响应 hashchange），
// 本桥只做上报，职责单一，不在页面新增视觉元素。
import { emit } from "@tauri-apps/api/event";

declare global {
  interface Window {
    __P2P_CONTROL_BRIDGE__?: boolean;
  }
}

// "#/chat" -> "chat"；"" / "#" / "#/" -> "dashboard"（menu.def.ts 的 index 路由）。
export function normalizeRoute(hash: string): string {
  const path = hash.replace(/^#\/?/, "").split("?")[0];
  return path === "" ? "dashboard" : path;
}

export function installControlBridge(): void {
  if (window.__P2P_CONTROL_BRIDGE__) return;
  window.__P2P_CONTROL_BRIDGE__ = true;
  const report = (): void => {
    emit("control-route", { route: normalizeRoute(window.location.hash) }).catch(
      (err: unknown) => {
        // 失败可观测：health 的 route 会停留在旧值，不允许静默丢
        console.warn("[control-bridge] 路由上报失败:", err);
      },
    );
  };
  window.addEventListener("hashchange", report);
  report();
}
