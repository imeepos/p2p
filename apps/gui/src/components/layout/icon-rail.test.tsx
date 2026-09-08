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
  it("一级入口与 menu.def 注册一一对应（F15 起 6 项），顺序一致", () => {
    renderRail("/chat");
    const links = railLinks();
    expect(links).toHaveLength(MENU_ENTRIES.length);
    links.forEach((link, index) => {
      expect(link.getAttribute("href")).toBe(MENU_ENTRIES[index].path);
      expect(link.getAttribute("aria-label")).toBe(i18n.t(MENU_ENTRIES[index].titleKey));
    });
  });

  it("F15：消息中心与协议文档常驻 rail，设置仍沉底", () => {
    renderRail("/chat");
    const hrefs = railLinks().map((link) => link.getAttribute("href"));
    expect(hrefs).toContain("/messages");
    expect(hrefs).toContain("/docs");
    expect(hrefs.indexOf("/messages")).toBeLessThan(hrefs.indexOf("/settings"));
    expect(hrefs.indexOf("/docs")).toBeLessThan(hrefs.indexOf("/settings"));
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

  it("设置入口唯一：头像入口已删，rail 内仅 nav 持有链接", () => {
    renderRail("/chat");
    const aside = screen.getByRole("complementary");
    const settingsLinks = within(aside)
      .getAllByRole("link")
      .filter((link) => link.getAttribute("href") === "/settings");
    expect(settingsLinks).toHaveLength(1);
    const nav = screen.getByRole("navigation");
    const outsideNav = within(aside)
      .getAllByRole("link")
      .filter((link) => !nav.contains(link));
    expect(outsideNav).toHaveLength(0);
  });

  it("rail 随主题自适应：底色用语义 sidebar 令牌，不再固定深色 wx-rail", () => {
    renderRail("/chat");
    const aside = screen.getByRole("complementary");
    expect(aside.className).toContain("bg-sidebar");
    expect(aside.className).not.toContain("wx-rail");
  });

  it("标题栏红绿灯已上移至同色条：nav 顶部常规留白（pt-4，类名即约定）", () => {
    renderRail("/chat");
    const nav = screen.getByRole("navigation");
    expect(nav.className).toContain("pt-4");
    expect(nav.className).not.toContain("pt-10");
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

describe("rail 消息中心待处理邀请角标（F15）", () => {
  it("角标与顶栏铃铛同源：两类 in 向 pending 之和；归零后消失", () => {
    useChatStore.setState({
      invites: [
        { peerId: "p-in", nickname: "甲", addrs: [], note: null, direction: "in", tsMs: 1, delivered: true },
        { peerId: "p-out", nickname: "丙", addrs: [], note: null, direction: "out", tsMs: 3, delivered: true },
      ],
      groupInvites: [
        { id: "gi-1", groupId: "g-1", groupName: "群", owner: "o", inviter: "i", invitee: "self", note: null, direction: "in", state: "pending", tsMs: 1, delivered: true },
      ],
    });
    renderRail("/chat");
    const badge = screen.getByTestId("rail-badge-/messages");
    expect(badge.textContent).toBe("2");
    expect(badge.getAttribute("aria-label")).toBe(i18n.t("messages.badgeAria", { count: 2 }));
    cleanup();

    useChatStore.setState({ invites: [], groupInvites: [] });
    renderRail("/chat");
    expect(screen.queryByTestId("rail-badge-/messages")).toBeNull();
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
