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

  it("启动后渲染出布局与内容，而非空白", async () => {
    const host = await bootApp();
    await vi.waitFor(
      () => {
        expect(host.querySelector("main")).not.toBeNull();
      },
      { timeout: 5000 },
    );
    expect(host.innerHTML.length).toBeGreaterThan(100);
    expect(host.textContent).not.toContain("界面出错了");
  });

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
