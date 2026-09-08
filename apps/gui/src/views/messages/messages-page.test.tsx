import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
    chatInviteCancel: vi.fn(),
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
    chatInviteCancel: mocks.chatInviteCancel,
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
  it("分区头：类型图标色 chip + 待处理计数角标（群=primary 绿，好友=info 蓝）", async () => {
    renderPage();
    await screen.findByTestId("messages-group-row-gi-1");
    // 群分区：pending 1 条（gi-1），gi-2 已终态不计入
    const groupCount = screen.getByTestId("section-count-primary");
    expect(groupCount.textContent).toBe("1");
    // 好友分区：收件箱即待处理集，in+out 共 2 条
    const friendCount = screen.getByTestId("section-count-info");
    expect(friendCount.textContent).toBe("2");
  });

  it("待处理视图默认只显示 pending 行，终态行移入历史视图", async () => {
    renderPage();
    const rowIn = await screen.findByTestId("messages-group-row-gi-1");
    expect(rowIn.textContent).toContain("项目组");
    expect(rowIn.textContent).toContain("收到的");
    expect(rowIn.textContent).toContain("待处理");
    expect(rowIn.textContent).toContain("备注：周末副本");
    expect(screen.getByTestId("messages-group-accept-gi-1")).toBeTruthy();
    // out+accepted 是终态，默认待处理视图不显示
    expect(screen.queryByTestId("messages-group-row-gi-2")).toBeNull();
    // 好友 out 向仍在待处理集（等待对方处理，可撤回）
    expect(await screen.findByTestId("messages-friend-row-" + PEER_OUT)).toBeTruthy();
    // 切到历史视图：终态行出现，pending 行退场，计数角标隐藏
    fireEvent.click(screen.getByTestId("segmented-history"));
    const rowOut = await screen.findByTestId("messages-group-row-gi-2");
    expect(rowOut.textContent).toContain("发出的");
    expect(rowOut.textContent).toContain("已同意");
    expect(screen.queryByTestId("messages-group-accept-gi-2")).toBeNull();
    expect(screen.queryByTestId("messages-group-row-gi-1")).toBeNull();
    expect(screen.queryByTestId("section-count-primary")).toBeNull();
  });

  it("历史视图：状态筛选 chips 过滤群终态，好友分区显示历史空态", async () => {
    renderPage();
    fireEvent.click(await screen.findByTestId("segmented-history"));
    expect(await screen.findByTestId("messages-group-row-gi-2")).toBeTruthy();
    expect(screen.getByTestId("segmented-all").getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(screen.getByTestId("segmented-rejected"));
    expect(screen.queryByTestId("messages-group-row-gi-2")).toBeNull();
    fireEvent.click(screen.getByTestId("segmented-accepted"));
    expect(screen.getByTestId("messages-group-row-gi-2")).toBeTruthy();
    // 好友收件箱即待处理集（契约无 state 字段），历史视图为显式空态
    expect(screen.getByText("好友邀请暂无历史记录")).toBeTruthy();
    expect(screen.getByText("好友邀请收件箱仅保留待处理条目，处理后即从列表移除")).toBeTruthy();
  });

  it("待处理计数渲染在分段控件标签上（群 pending + 好友全集）", async () => {
    renderPage();
    await screen.findByTestId("messages-group-row-gi-1");
    const pending = screen.getByTestId("segmented-pending");
    expect(pending.textContent).toContain("待处理");
    expect(pending.textContent).toContain("3");
  });

  it("好友列表：in 向行内备注昵称输入与同意/拒绝，out 向无同意入口", async () => {
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

  it("邀请卡：人可读名为标题，PeerId 缩略（前 6 后 4）+ 复制（F21）", async () => {
    renderPage();
    const rowOut = await screen.findByTestId("messages-friend-row-" + PEER_OUT);
    expect(rowOut.textContent).toContain("小圆");
    expect(rowOut.textContent).toContain("peer-o…bbbb");
    expect(rowOut.textContent).not.toContain(PEER_OUT);
    expect(
      within(rowOut).getByRole("button", { name: "复制" }),
    ).toBeTruthy();
    const rowIn = await screen.findByTestId("messages-friend-row-" + PEER_IN);
    expect(
      within(rowIn).getByRole("button", { name: "复制" }),
    ).toBeTruthy();
    const groupRow = screen.getByTestId("messages-group-row-gi-1");
    expect(
      within(groupRow).getByRole("button", { name: "复制" }),
    ).toBeTruthy();
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
