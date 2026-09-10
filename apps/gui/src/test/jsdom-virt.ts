// jsdom 虚拟化测试基座（react-virtuoso + react-window）。
// 压力测试文件显式引入；setup.ts 不全局注入，避免改变其余测试语义。
//
// 实测结论（2026-09-10 spike 沉淀）：
// - react-virtuoso 4.18 在 jsdom 渲染为 0 items：RO 回调有 offsetParent!==null
//   守卫、scrollTo 有 offsetHeight===0 守卫。官方出路是 VirtuosoMockContext
//   （viewportHeight/itemHeight 走 fixedItemHeight 内部路径，绕过 DOM 测量）。
// - firstItemIndex 在前插时应「减小」新增条数；配合 mock context 视口内容
//   等价锚定（scrollTop 平移 K*itemH），是旧 UX5 手工补偿的替代物。
// - scrollToIndex/followOutput 的命令式滚动在 offsetHeight===0 时静默跳过，
//   因此 offsetHeight/getBoundingClientRect 桩是必需项。
// - 真浏览器 scroll 事件异步派发；这里同样入队延迟派发，避免与 React 提交
//   交错造成 virtuoso 锚定竞态。
import { vi } from "vitest";
import { VirtuosoMockContext, type VirtuosoMockContextValue } from "react-virtuoso";

export const VIRT_VIEWPORT_HEIGHT = 600;
export const VIRT_ITEM_HEIGHT = 48;

export const VIRT_MOCK_CONTEXT: VirtuosoMockContextValue = {
  viewportHeight: VIRT_VIEWPORT_HEIGHT,
  itemHeight: VIRT_ITEM_HEIGHT,
};

export { VirtuosoMockContext };

const pendingScrolls: Element[] = [];

function scrollBy(el: HTMLElement, top: number, relative: boolean): void {
  const current = Number(el.scrollTop) || 0;
  Object.defineProperty(el, "scrollTop", {
    value: relative ? current + top : top,
    writable: true,
    configurable: true,
  });
  pendingScrolls.push(el);
}

let installed = false;

export function installJsdomVirtPolyfill(): void {
  if (installed) return;
  installed = true;

  // 两个库都按 ResizeObserver 存在与与否分支；mock context 下不依赖其回调
  // 产物，仅提供构造器防止 typeof 检查走入 SSR 分支。
  class MockResizeObserver {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  vi.stubGlobal("ResizeObserver", MockResizeObserver);

  Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
    configurable: true,
    get(this: HTMLElement) {
      const own = this.getAttribute("data-vh");
      return own !== null ? Number(own) : VIRT_VIEWPORT_HEIGHT;
    },
  });

  (Element.prototype as { getBoundingClientRect?: () => DOMRect }).getBoundingClientRect =
    function () {
      return {
        width: 400,
        height: VIRT_VIEWPORT_HEIGHT,
        x: 0,
        y: 0,
        top: 0,
        left: 0,
        right: 400,
        bottom: VIRT_VIEWPORT_HEIGHT,
        toJSON: () => ({}),
      } as DOMRect;
    } as typeof Element.prototype.getBoundingClientRect;

  Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
    configurable: true,
    get(this: HTMLElement) {
      const own = this.getAttribute("data-scroll-height");
      return own !== null ? Number(own) : 0;
    },
  });

  // jsdom 未实现元素级滚动；polyfill 成「写 scrollTop + 排队 scroll 事件」，
  // 让 virtuoso 的 scrollToIndex/scrollBy 与真浏览器走同一状态回路。
  Element.prototype.scrollTo = function (this: HTMLElement, arg?: ScrollToOptions) {
    if (arg && typeof arg.top === "number") scrollBy(this, arg.top, false);
  } as unknown as typeof Element.prototype.scrollTo;
  Element.prototype.scrollBy = function (this: HTMLElement, arg?: ScrollToOptions) {
    if (arg && typeof arg.top === "number") scrollBy(this, arg.top, true);
  } as unknown as typeof Element.prototype.scrollBy;
  Element.prototype.scrollIntoView = function () {} as unknown as typeof Element.prototype.scrollIntoView;
}

/** 设置 scrollTop 并按真浏览器语义延迟派发 scroll 事件（settleVirt 冲刷）。 */
export function setScrollTop(el: Element, top: number): void {
  scrollBy(el as HTMLElement, top, false);
}

/** 冲刷排队 scroll 事件并等待 virtuoso 状态机经过若干拍宏任务。 */
export async function settleVirt(ticks = 6, tickMs = 10): Promise<void> {
  for (let i = 0; i < ticks; i++) {
    await new Promise((r) => setTimeout(r, tickMs));
    flush();
  }
  await new Promise((r) => setTimeout(r, 10));
}

function flush(): void {
  const batch = pendingScrolls.splice(0);
  for (const el of batch) el.dispatchEvent(new Event("scroll"));
}

/** 轮询等待条件成立（每拍冲刷排队 scroll 事件）。
 * virtuoso 的 startReached/endReached 走 200ms 节流流，固定时长的
 * settleVirt 与之竞态；条件轮询保证确定性。 */
export async function settleUntil(
  cond: () => boolean,
  budgetMs = 2000,
  stepMs = 50,
): Promise<void> {
  const deadline = Date.now() + budgetMs;
  for (;;) {
    await new Promise((r) => setTimeout(r, stepMs));
    flush();
    if (cond() || Date.now() >= deadline) return;
  }
}
