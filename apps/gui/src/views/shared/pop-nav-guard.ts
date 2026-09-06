// F05 兜底层：delta==null 的 POP 导航（手动改 hash、经无 router 状态条目的
// 前进后退）会绕过 useBlocker——@remix-run/router 对这类 POP 只 warn 后直接
// 放行（"blocker ... fail silently"），脏草稿随之静默丢失。
// 本模块在 import 期挂 window popstate 首环：守卫 hook 有脏且 delta 不可判时
// 先回滚 URL 再弹确认；router 随后读到的是回滚后的同址 POP，不再丢稿。
// 注册顺序即调用顺序：import 早于 RouterProvider 的 router.initialize()。
export type PopStateHandler = (event: PopStateEvent) => void;

let handler: PopStateHandler | null = null;

window.addEventListener("popstate", (event) => {
  handler?.(event);
});

// 守卫组件挂载即接管、卸载让位；UnsavedRouteGuard 为路由元素级，单实例存活
export function setPopStateHandler(next: PopStateHandler | null): void {
  handler = next;
}

// history.state.idx 是 router delta 的来源；缺 idx 即 delta 不可判（router 不拦）
export function historyIdxOf(state: unknown): number | null {
  if (state === null || typeof state !== "object") return null;
  const idx = (state as { idx?: unknown }).idx;
  return typeof idx === "number" ? idx : null;
}
