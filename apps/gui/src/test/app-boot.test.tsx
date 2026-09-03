import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ErrorBoundary } from "@/components/feedback/error-boundary";

// 启动冒烟：整应用在 jsdom 里真实挂载一次，锁死"构建绿但白屏"这类整树崩溃。
// IPC 经 VITE_MOCK_IPC 走 mockBackend，不依赖 Tauri 运行时。
// 注意：必须在动态 import("../main") 之前 stub，ipc.ts 在模块求值期读该值。
vi.stubEnv("VITE_MOCK_IPC", "1");

async function bootApp(): Promise<HTMLElement> {
  const host = document.createElement("div");
  host.id = "root";
  document.body.appendChild(host);
  await import("../main");
  return host;
}

describe("app boot smoke", () => {
  afterEach(() => {
    // bootApp 手工挂载的 host 不归 RTL cleanup 管，防跨测试 DOM 泄漏
    document.body.innerHTML = "";
  });

  // 启动 + 全路由冒烟：main.tsx 模块只求值一次（双 boot 无效），
  // 路由遍历必须与启动同用例。原启动冒烟只踩默认路由，
  // 放过过 relay 页整树崩溃（FormProvider 外解构 useFormContext 的
  // control，2026-09-03 用户实测白屏）；逐路由真实导航兜住同类事故。
  it("启动后逐路由渲染，无整树崩溃兜底", async () => {
    const host = await bootApp();
    await vi.waitFor(
      () => {
        expect(host.querySelector("main")).not.toBeNull();
      },
      { timeout: 5000 },
    );
    expect(host.innerHTML.length).toBeGreaterThan(100);
    expect(host.textContent).not.toContain("界面出错了");

    // 路由就绪才出现的标记：锁"数据加载完成"而非骨架屏，
    // relay/settings 的崩溃恰好发生在配置就绪挂载表单卡那一刻。
    const readyMarkers: Record<string, string> = {
      "#/relay": "中继地址配置",
      "#/settings": "局域网发现（mDNS）",
    };
    for (const hash of [
      "#/peers",
      "#/discovery",
      "#/relay",
      "#/events",
      "#/settings",
      "#/diagnostics",
    ]) {
      window.location.hash = hash;
      await vi.waitFor(
        () => {
          expect(host.querySelector("main")).not.toBeNull();
          const marker = readyMarkers[hash];
          if (marker) expect(host.textContent).toContain(marker);
          expect(host.textContent).not.toContain("界面出错了");
        },
        { timeout: 3000 },
      );
    }
    window.location.hash = "#/";
  }, 30000);

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
