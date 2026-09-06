import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";

import "@/i18n";
import i18n from "@/i18n";

import { CommandPalette } from "./command-palette";
import { requestOpenCommandPalette } from "./palette-bus";

const t = i18n.t.bind(i18n);

// cmdk 依赖 ResizeObserver/scrollIntoView，jsdom 未实现（对齐 command-palette.test 桩）
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
(window as unknown as { ResizeObserver: unknown }).ResizeObserver = ResizeObserverStub;
if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {};
}

afterEach(() => cleanup());

describe("命令面板 llm-share 入口（LSG3：rail 不动，面板可达）", () => {
  it("工具组下注册 /llm-share 项，点击后关闭面板", async () => {
    const onOpenChange = vi.fn();
    render(
      <MemoryRouter>
        <CommandPalette open={false} onOpenChange={onOpenChange} />
      </MemoryRouter>,
    );
    act(() => {
      requestOpenCommandPalette();
    });
    expect(await screen.findByRole("dialog")).toBeTruthy();
    expect(screen.getByText(t("llmShare.paletteGroup"))).toBeTruthy();
    fireEvent.click(screen.getByRole("option", { name: t("llmShare.title") }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });
});
