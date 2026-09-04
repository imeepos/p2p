import { render, screen } from "@testing-library/react";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

import { ErrorBoundary } from "@/components/feedback/error-boundary";

// 启动冒烟：整应用在 jsdom 里真实挂载一次，锁死"构建绿但白屏"这类整树崩溃。
// IPC 经 VITE_MOCK_IPC 走 mockBackend，不依赖 Tauri 运行时。
// 注意：必须在动态 import("../main") 之前 stub，ipc.ts 在模块求值期读该值。
vi.stubEnv("VITE_MOCK_IPC", "1");

// IM-T53 负载加固：原单用例承载启动+7 路由遍历，负载下撞 30s 级超时雪崩。
// main.tsx 模块只求值一次（双 boot 无效），故 boot 收进 beforeAll 做一次，
// 逐路由拆成独立用例只做 hash 导航，单路由慢/红不再拖垮整链。
const BOOT_TIMEOUT = 30_000;
const ROUTE_TEST_TIMEOUT = 20_000;
const ROUTE_WAIT_TIMEOUT = 10_000;

let host: HTMLElement;

beforeAll(async () => {
  host = document.createElement("div");
  host.id = "root";
  document.body.appendChild(host);
  await import("../main");
  await vi.waitFor(
    () => {
      expect(host.querySelector("main")).not.toBeNull();
    },
    { timeout: BOOT_TIMEOUT },
  );
}, BOOT_TIMEOUT + 10_000);

afterAll(() => {
  // boot 手工挂载的 host 不归 RTL cleanup 管，收尾清场防跨文件泄漏
  document.body.innerHTML = "";
  window.location.hash = "#/";
});

describe("app boot smoke", () => {
  it("启动挂载默认路由，非空渲染且无兜底", () => {
    expect(host.innerHTML.length).toBeGreaterThan(100);
    expect(host.textContent).not.toContain("界面出错了");
  });
});

// 路由就绪才出现的标记：锁"数据加载完成"而非骨架屏，
// relay/settings 的崩溃恰好发生在配置就绪挂载表单卡那一刻
// （原启动冒烟只踩默认路由，放过过 relay 页整树崩溃，2026-09-03 用户实测白屏）。
const routes: Array<[string, string | null]> = [
  ["#/peers", null],
  ["#/discovery", null],
  ["#/relay", "中继地址配置"],
  ["#/chat", "暂无好友"],
  ["#/events", null],
  ["#/settings", "局域网发现（mDNS）"],
  ["#/diagnostics", null],
];

describe.each(routes)("app boot route %s", (hash, marker) => {
  it("逐路由渲染，无整树崩溃兜底", async () => {
    window.location.hash = hash;
    await vi.waitFor(
      () => {
        expect(host.querySelector("main")).not.toBeNull();
        if (marker) expect(host.textContent).toContain(marker);
        expect(host.textContent).not.toContain("界面出错了");
      },
      { timeout: ROUTE_WAIT_TIMEOUT },
    );
    expect(host.innerHTML.length).toBeGreaterThan(100);
  }, ROUTE_TEST_TIMEOUT);
});

describe("app boot error boundary", () => {
  it("渲染崩溃时 ErrorBoundary 显示兜底文案而非白屏", () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    function Bomb(): never {
      throw new Error("boot-smoke-boom");
    }

    render(
      <ErrorBoundary>
        <Bomb />
      </ErrorBoundary>,
    );
    expect(screen.getByText(/界面出错了/)).toBeInTheDocument();
    expect(screen.getByText(/boot-smoke-boom/)).toBeInTheDocument();
    consoleError.mockRestore();
  });
});
