import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";

import { MENU_ENTRIES } from "@/config/menu.def";
import { PALETTE_NAV_ENTRIES } from "@/config/palette-nav";
import "@/i18n";
import { useNodeStore } from "@/stores/node-store";
import {
  ACP_MANAGE_PALETTE_COUNT,
  CommandPalette,
  LLM_SHARE_PALETTE_COUNT,
} from "./command-palette";
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
  it("打开时列出全部导航注册项（节点组数据为空时不渲染）", async () => {
    renderPalette(true);
    expect(await screen.findByRole("dialog")).toBeTruthy();
    expect(screen.getAllByRole("option")).toHaveLength(
      PALETTE_NAV_ENTRIES.length +
        LLM_SHARE_PALETTE_COUNT +
        ACP_MANAGE_PALETTE_COUNT,
    );
  });

  it("底部快捷键说明与 rail 注册数同源（不硬编码 1..4）", async () => {
    renderPalette(true);
    await screen.findByRole("dialog");
    expect(screen.getByText("Cmd/Ctrl+K 打开命令面板")).toBeTruthy();
    // R2-25：提示数字取 menu.def 注册数（热键实现同一上界），rail 扩到 6
    // 后不再出现过期的「1..4」
    expect(
      screen.getByText("Cmd/Ctrl+1.." + MENU_ENTRIES.length + " 切换一级入口"),
    ).toBeTruthy();
    expect(screen.queryByText(/1..4 切换/)).toBeNull();
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

  // R2-17 回归：节点项缩略与全站同口径（前 6…后 4 + 悬停完整），不再 16 位前缀。
  it("节点项渲染共享缩略口径并保留整行点击复制语义", async () => {
    const fullPeerId = "vKLTAv6c8dEfGhIjKlMnOpQrStUvWxyzAbCdEfGhIjkl";
    useNodeStore.setState({
      peers: {
        p1: {
          peerId: fullPeerId,
          addrs: ["/ip4/127.0.0.1/tcp/4001"],
          source: "mdns",
          connected: true,
          lastSeenMs: 1,
          hops: [],
        },
      },
    });
    try {
      renderPalette(true);
      await screen.findByRole("dialog");
      const cell = screen.getByTitle(fullPeerId);
      expect(cell).toHaveTextContent(
        fullPeerId.slice(0, 6) + "…" + fullPeerId.slice(-4),
      );
      expect(screen.getByText("复制 PeerId")).toBeInTheDocument();
    } finally {
      useNodeStore.setState({ peers: {} });
    }
  });
});
