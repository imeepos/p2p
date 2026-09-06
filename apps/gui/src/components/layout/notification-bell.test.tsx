import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { describe, expect, it } from "vitest";
import type { FriendInviteJson, GroupInviteJson } from "@/lib/ipc-types";

import "@/i18n";

// 顶栏铃铛（IMC3 需求 2）：徽标数 = 入群邀请 in 向 pending + 好友邀请 in 向；
// 99+ 截断；归零隐藏；点击进 /messages。

import { useChatStore } from "@/stores/chat-store";
import { NotificationBell } from "./notification-bell";

function groupInvite(direction: "in" | "out", state: GroupInviteJson["state"]): GroupInviteJson {
  return {
    id: "gi-" + direction + state,
    groupId: "g-1",
    groupName: "项目组",
    owner: "o",
    inviter: "i",
    invitee: "e",
    note: null,
    direction,
    state,
    tsMs: 1,
    delivered: true,
  };
}

function friendInvite(direction: "in" | "out"): FriendInviteJson {
  return {
    peerId: "p-" + direction,
    nickname: "甲",
    addrs: [],
    note: null,
    direction,
    tsMs: 1,
    delivered: true,
  };
}

function LocationProbe() {
  const location = useLocation();
  return <div data-testid="loc">{location.pathname}</div>;
}

function renderBell() {
  render(
    <MemoryRouter initialEntries={["/chat"]}>
      <LocationProbe />
      <NotificationBell />
    </MemoryRouter>,
  );
}

describe("NotificationBell 徽标计数", () => {
  it("两类 in 向 pending 求和；out 与非 pending 不计", () => {
    useChatStore.setState({
      groupInvites: [
        groupInvite("in", "pending"),
        groupInvite("in", "accepted"),
        groupInvite("out", "pending"),
      ],
      invites: [friendInvite("in"), friendInvite("out")],
    });
    renderBell();
    expect(screen.getByTestId("notification-badge").textContent).toBe("2");
    cleanup();
  });

  it("归零后徽标消失", () => {
    useChatStore.setState({ groupInvites: [], invites: [] });
    renderBell();
    expect(screen.queryByTestId("notification-badge")).toBeNull();
    cleanup();
  });

  it("超过 99 显示 99+", () => {
    const many = Array.from({ length: 120 }, (_, i) => ({
      ...friendInvite("in"),
      peerId: "p-" + i,
    }));
    useChatStore.setState({ groupInvites: [], invites: many });
    renderBell();
    expect(screen.getByTestId("notification-badge").textContent).toBe("99+");
    cleanup();
  });

  it("点击铃铛进入 /messages", () => {
    useChatStore.setState({ groupInvites: [], invites: [] });
    renderBell();
    fireEvent.click(screen.getByTestId("notification-bell"));
    expect(screen.getByTestId("loc").textContent).toBe("/messages");
  });
});
