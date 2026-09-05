import { cleanup, render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it } from "vitest";

import i18n from "@/i18n";
import { MENU_ENTRIES } from "@/config/menu.def";
import { IconRail } from "./icon-rail";

function renderRail(initial: string) {
  return render(
    <MemoryRouter initialEntries={[initial]}>
      <IconRail />
    </MemoryRouter>,
  );
}

function railLinks(): HTMLElement[] {
  return within(screen.getByRole("navigation")).getAllByRole("link");
}

afterEach(() => cleanup());

describe("IconRail（1.1 rail 规格）", () => {
  it("恰 4 个一级入口，顺序为聊天/通讯录/网络/设置", () => {
    renderRail("/chat");
    const links = railLinks();
    expect(links).toHaveLength(4);
    links.forEach((link, index) => {
      expect(link.getAttribute("href")).toBe(MENU_ENTRIES[index].path);
      expect(link.getAttribute("aria-label")).toBe(i18n.t(MENU_ENTRIES[index].titleKey));
    });
  });

  it("设置沉底：rail 末位入口是 /settings", () => {
    renderRail("/chat");
    const links = railLinks();
    expect(links[links.length - 1]?.getAttribute("href")).toBe("/settings");
  });

  it("仅图标呈现：链接内无文字节点，图标可访问名来自 aria-label", () => {
    renderRail("/chat");
    railLinks().forEach((link) => {
      expect(link.querySelector("svg")).not.toBeNull();
      expect(link.textContent?.trim()).toBe("");
    });
  });

  it("选中态高亮当前路由，其余不高亮", () => {
    renderRail("/network/peers");
    const links = railLinks();
    // NavLink 激活时输出 aria-current="page"（高亮类名经 isActive 拼接）
    const active = links.filter(
      (link) => link.getAttribute("aria-current") === "page",
    );
    expect(active).toHaveLength(1);
    expect(active[0]?.getAttribute("href")).toBe("/network");
  });
});
