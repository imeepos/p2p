import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it } from "vitest";

import { CommandPalette } from "@/components/command-palette/command-palette";
import { ThemeProvider } from "@/theme/theme-provider";
import "@/i18n";
import { Topbar } from "./topbar";

// cmdk 依赖 ResizeObserver 测量与 scrollIntoView 滚动，jsdom 均未实现：
// 仅在测试内补最小桩，不改变被测行为
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
(window as unknown as { ResizeObserver: unknown }).ResizeObserver = ResizeObserverStub;
if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {};
}

function stubPlatform(value: string) {
  Object.defineProperty(navigator, "platform", { configurable: true, value });
}

function renderTopbar() {
  return render(
    <ThemeProvider>
      <MemoryRouter>
        <Topbar />
      </MemoryRouter>
    </ThemeProvider>,
  );
}

// 与 app-layout 等价的受控壳：验证顶栏入口能真实打开面板
function Shell() {
  const [open, setOpen] = useState(false);
  return (
    <ThemeProvider>
      <MemoryRouter>
        <Topbar />
        <CommandPalette open={open} onOpenChange={setOpen} />
      </MemoryRouter>
    </ThemeProvider>
  );
}

afterEach(() => {
  cleanup();
  stubPlatform("");
});

describe("Topbar 命令面板入口", () => {
  it("渲染命令按钮；非 Apple 平台徽标为 Ctrl+K", () => {
    stubPlatform("Win32");
    renderTopbar();
    const button = screen.getByRole("button", { name: "命令面板" });
    expect(button.textContent).toContain("Ctrl+K");
  });

  it("Apple 平台徽标为 Cmd 系（\u2318K）", () => {
    stubPlatform("MacIntel");
    renderTopbar();
    expect(screen.getByRole("button", { name: "命令面板" }).textContent).toContain(
      "\u2318K",
    );
  });

  it("点击命令按钮打开命令面板", async () => {
    render(<Shell />);
    fireEvent.click(screen.getByRole("button", { name: "命令面板" }));
    expect(await screen.findByRole("dialog")).toBeTruthy();
  });
});

// macOS Overlay 标题栏：顶栏即唯一标题栏，header 空白区带拖拽属性。
// 紧凑化后标题文本与节点启停文字按钮不再存在，仅保留右侧图标控件。
describe("Topbar 紧凑标题栏（Overlay）", () => {
  it("header 带 data-tauri-drag-region，无标题文本与启停按钮", () => {
    renderTopbar();
    const header = screen.getByRole("banner");
    expect(header.hasAttribute("data-tauri-drag-region")).toBe(true);
    expect(screen.queryByText("p2p-console")).toBeNull();
    expect(screen.queryByTestId("topbar-stop-node")).toBeNull();
  });
});
