import { afterEach } from "vitest";

import { cleanup } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

// vitest 未开 globals，RTL 无法自动注册 cleanup
afterEach(() => cleanup());

// jsdom 没有 matchMedia，ThemeProvider 的 system 监听依赖它
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  }),
});
