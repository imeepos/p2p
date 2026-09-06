import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FriendInviteJson, GroupInviteJson } from "@/lib/ipc-types";

import "@/i18n";

// 消息中心（IMC3 需求 2）：两组列表与方向/状态徽章、行内操作与失败上浮、
// 行点击跳对应会话、空态文案。IPC mock 命令名与契约逐字一致。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    chatInvitesList: vi.fn(),
    chatGroupInvitesList: vi.fn(),
    chatGroupInviteAccept: vi.fn(),
    chatGroupInviteReject: vi.fn(),
    chatInviteAccept: vi.fn(),
    chatInviteReject: vi.fn(),
    chatFriendsList: vi.fn(),
    chatHistory: vi.fn(),
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatInvitesList: mocks.chatInvitesList,
    chatGroupInvitesList: mocks.chatGroupInvitesList,
    chatGroupInviteAccept: mocks.chatGroupInviteAccept,
    chatGroupInviteReject: mocks.chatGroupInviteReject,
    chatInviteAccept: mocks.chatInviteAccept,
    chatInviteReject: mocks.chatInviteReject,
    chatFriendsList: mocks.chatFriendsList,
    chatHistory: mocks.chatHistory,
  },
}));

import { useChatStore } from "@/stores/chat-store";
import { MessagesPage } from "./messages-page";

const PEER_IN = "peer-in-aaaaaaaa";
const PEER_OUT = "peer-out-bbbbbbbb";

const groupIn: GroupInviteJson = {
  id: "gi-1",
  groupId: "g-1",
  groupName: "项目组",
  owner: "owner-x",
  inviter: "inviter-x",
  invitee: "self",
  note: "周末副本",
  direction: "in",
  state: "pending",
  tsMs: 1_700_000_000_000,
  delivered: true,
};

const groupOut: GroupInviteJson = {
  ...groupIn,
  id: "gi-2",
  groupId: "g-2",
  groupName: "钓鱼群",
  inviter: "self",
  invitee: "friend-y",
  direction: "out",
  state: "accepted",
  tsMs: 1_700_000_001_000,
};

const friendIn: FriendInviteJson = {
  peerId: PEER_IN,
  nickname: "阿北",
  addrs: [],
  note: "老朋友",
  direction: "in",
  tsMs: 1_700_000_002_000,
  delivered: true,
};

const friendOut: FriendInviteJson = {
  peerId: PEER_OUT,
  nickname: "小圆",
  addrs: [],
  note: null,
  direction: "out",
  tsMs: 1_700_000_003_000,
  delivered: false,
};

function LocationProbe() {
  const location = useLocation();
  return <div data-testid="loc">{location.pathname + location.search}</div>;
}

function renderPage() {
  render(
    <MemoryRouter initialEntries={["/messages"]}>
      <LocationProbe />
      <MessagesPage />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  for (const fn of Object.values(mocks)) {
    if (typeof fn === "function") (fn as ReturnType<typeof vi.fn>).mockReset();
  }
  mocks.chatInvitesList.mockResolvedValue([friendIn, friendOut]);
  mocks.chatGroupInvitesList.mockResolvedValue([groupIn, groupOut]);
  mocks.chatFriendsList.mockResolvedValue([]);
  mocks.chatHistory.mockResolvedValue([]);
  useChatStore.setState({
    groupInvites: [],
    groupInvitesLoaded: false,
    groupInvitesError: null,
    invites: [],
    friends: [],
    friendsLoaded: true,
    friendsError: null,
  });
});

describe("消息中心两组列表", () => {
  it("入组列表：方向/状态徽章、备注齐备，out+accepted 无行内按钮", async () => {
    renderPage();
    const rowIn = await screen.findByTestId("messages-group-row-gi-1");
    expect(rowIn.textContent).toContain("项目组");
    expect(rowIn.textContent).toContain("收到的");
    expect(rowIn.textContent).toContain("待处理");
    expect(rowIn.textContent).toContain("备注：周末副本");
    expect(screen.getByTestId("messages-group-accept-gi-1")).toBeTruthy();
    const rowOut = screen.getByTestId("messages-group-row-gi-2");
    expect(rowOut.textContent).toContain("发出的");
    expect(rowOut.textContent).toContain("已同意");
    expect(screen.queryByTestId("messages-group-accept-gi-2")).toBeNull();
  });

  it("好友列表：in 向行内备注昵称输入与同意/拒绝，out 向无操作", async () => {
    renderPage();
    const rowIn = await screen.findByTestId("messages-friend-row-" + PEER_IN);
    expect(rowIn.textContent).toContain("收到的");
    expect(screen.getByTestId("messages-friend-nickname-" + PEER_IN)).toBeTruthy();
    expect(screen.getByTestId("messages-friend-accept-" + PEER_IN)).toBeTruthy();
    const rowOut = screen.getByTestId("messages-friend-row-" + PEER_OUT);
    expect(rowOut.textContent).toContain("发出的");
    expect(screen.queryByTestId("messages-friend-accept-" + PEER_OUT)).toBeNull();
  });

  it("空态：两组列表各自空文案", async () => {
    mocks.chatInvitesList.mockResolvedValue([]);
    mocks.chatGroupInvitesList.mockResolvedValue([]);
    renderPage();
    await waitFor(() => expect(screen.getByText("暂无入群邀请")).toBeTruthy());
    expect(screen.getByText("暂无好友邀请")).toBeTruthy();
  });

  it("群邀请行内同意失败：原文上浮 role=alert", async () => {
    mocks.chatGroupInviteAccept.mockRejectedValue(new Error("名单版本冲突"));
    renderPage();
    fireEvent.click(await screen.findByTestId("messages-group-accept-gi-1"));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("同意失败：");
    expect(alert.textContent).toContain("名单版本冲突");
  });

  it("群邀请行内拒绝成功：带邀请 id 调用契约命令", async () => {
    mocks.chatGroupInviteReject.mockResolvedValue(undefined);
    renderPage();
    fireEvent.click(await screen.findByTestId("messages-group-reject-gi-1"));
    await waitFor(() =>
      expect(mocks.chatGroupInviteReject).toHaveBeenCalledWith("gi-1", null),
    );
  });

  it("好友邀请行内同意：沿用备注昵称口径（trim 后传入）", async () => {
    mocks.chatInviteAccept.mockResolvedValue({ peerId: PEER_IN, nickname: "阿北", addrs: [] });
    renderPage();
    fireEvent.click(await screen.findByTestId("messages-friend-accept-" + PEER_IN));
    await waitFor(() =>
      expect(mocks.chatInviteAccept).toHaveBeenCalledWith(PEER_IN, ""),
    );
  });

  it("群行点击跳群会话、好友行点击跳好友聊天", async () => {
    renderPage();
    fireEvent.click(await screen.findByTestId("messages-group-row-gi-1"));
    expect(screen.getByTestId("loc").textContent).toBe("/chat?group=g-1");
    fireEvent.click(screen.getByTestId("messages-friend-row-" + PEER_IN));
    expect(screen.getByTestId("loc").textContent).toBe("/chat?peer=" + PEER_IN);
  });

  it("列表加载失败原文上浮（群组错误行）", async () => {
    mocks.chatGroupInvitesList.mockRejectedValue(new Error("rpc down"));
    renderPage();
    const err = await screen.findByTestId("messages-group-list-error");
    expect(err.textContent).toContain("邀请列表加载失败：");
    expect(err.textContent).toContain("rpc down");
  });
});
