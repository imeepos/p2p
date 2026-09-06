// Tauri 运行环境探测：桌面 webview 由 Tauri v2 注入 __TAURI_INTERNALS__；
// 纯浏览器（mock dev / 浏览器预览）无此全局。桥接类能力据此走降级路径，
// 避免在无接收方的环境里反复报错刷屏（F26/F27）。
declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export function isTauriRuntime(): boolean {
  return (
    typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined
  );
}
