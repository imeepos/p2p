import { cleanup, render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it } from "vitest";

import i18n from "@/i18n";
import { useAcpStore } from "@/acp/acp-store";
import { MENU_ENTRIES } from "@/config/menu.def";
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";
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

describe("rail 聊天未读合计角标（§2.3）", () => {
  it("三来源未读求和呈现；归零后角标消失；≥100 显 99+", () => {
    useChatStore.setState({ unreadByPeer: { p1: 2, p2: 3 } });
    useGroupStore.setState({ unreadByGroup: { g1: 5 } });
    useAcpStore.setState({ unreadByEndpoint: { e1: 90 } });
    renderRail("/chat");
    const badge = screen.getByTestId("rail-badge-/chat");
    // 合计 100（2+3+5+90）≥100 → 显 99+
    expect(badge.textContent).toBe("99+");
    cleanup();

    useChatStore.setState({ unreadByPeer: {} });
    useGroupStore.setState({ unreadByGroup: {} });
    useAcpStore.setState({ unreadByEndpoint: {} });
    renderRail("/chat");
    expect(screen.queryByTestId("rail-badge-/chat")).toBeNull();
  });
});

describe("rail 通讯录待处理邀请角标（§3.2）", () => {
  it("in 向邀请计数呈现角标；out 向不计；归零后消失", () => {
    useChatStore.setState({
      invites: [
        { peerId: "p-in", nickname: "甲", addrs: [], note: null, direction: "in", tsMs: 1, delivered: true },
        { peerId: "p-in2", nickname: "乙", addrs: [], note: null, direction: "in", tsMs: 2, delivered: true },
        { peerId: "p-out", nickname: "丙", addrs: [], note: null, direction: "out", tsMs: 3, delivered: true },
      ],
    });
    renderRail("/chat");
    const badge = screen.getByTestId("rail-badge-/contacts");
    expect(badge.textContent).toBe("2");
    expect(badge.getAttribute("aria-label")).toBe(i18n.t("contacts.inviteBadge.aria", { count: 2 }));
    cleanup();

    useChatStore.setState({ invites: [] });
    renderRail("/chat");
    expect(screen.queryByTestId("rail-badge-/contacts")).toBeNull();
  });
});
