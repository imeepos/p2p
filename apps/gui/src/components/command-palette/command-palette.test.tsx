import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";

import { MENU_ENTRIES } from "@/config/menu.def";
import "@/i18n";
import { CommandPalette } from "./command-palette";
import { requestOpenCommandPalette } from "./palette-bus";

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



function renderPalette(open: boolean, onOpenChange = () => {}) {
  return render(
    <MemoryRouter>
      <CommandPalette open={open} onOpenChange={onOpenChange} />
    </MemoryRouter>,
  );
}

afterEach(() => cleanup());

describe("CommandPalette", () => {
  it("打开时列出全部注册路由菜单项", async () => {
    renderPalette(true);
    expect(await screen.findByRole("dialog")).toBeTruthy();
    expect(screen.getAllByRole("option")).toHaveLength(MENU_ENTRIES.length);
  });

  it("底部渲染修正后的快捷键说明（1..9，与热键实现一致）", async () => {
    renderPalette(true);
    await screen.findByRole("dialog");
    expect(screen.getByText("Cmd/Ctrl+K 打开命令面板")).toBeTruthy();
    expect(screen.getByText("Cmd/Ctrl+1..9 切换前 9 个页面")).toBeTruthy();
    expect(screen.getByText("Esc 关闭")).toBeTruthy();
  });

  it("事件总线的外部打开请求可打开面板（顶栏入口依赖此通道）", async () => {
    renderPalette(false);
    act(() => {
      requestOpenCommandPalette();
    });
    expect(await screen.findByRole("dialog")).toBeTruthy();
  });

  it("选择菜单项后通知关闭面板", async () => {
    const onOpenChange = vi.fn();
    renderPalette(true, onOpenChange);
    await screen.findByRole("dialog");
    fireEvent.click(screen.getAllByRole("option")[1]);
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });
});
