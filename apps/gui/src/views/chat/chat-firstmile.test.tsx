import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useEffect } from "react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ChatFriendJson,
  FriendInviteJson,
  GroupInviteJson,
} from "@/lib/ipc-types";
import { useAcpStore } from "@/acp/acp-store";
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";
import { pendingInviteItems } from "./use-pending-invites";
import { ChatPage } from "./chat-page";

import "@/i18n";

// UX-H 首公里（UX 审计 20260907 F01/F17）：新用户空态双 CTA 不断头 +
// 邀请发出后列表出现「等待对方同意」置灰占位（状态与通讯录/消息中心同源）。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    history: vi.fn<() => Promise<never[]>>(),
    invites: vi.fn<() => Promise<FriendInviteJson[]>>(),
    groupInvites: vi.fn<() => Promise<GroupInviteJson[]>>(),
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatHistory: mocks.history,
    chatInvitesList: mocks.invites,
    chatGroupInvitesList: mocks.groupInvites,
    // ChatPage 聚合挂载面：群/节点态存根（本文件不涉及其行为）
    groupList: vi.fn().mockResolvedValue([]),
    groupHistory: vi.fn().mockResolvedValue([]),
    nodeStatus: vi.fn().mockResolvedValue({ peerId: "self-peer", running: true }),
    onNodeEvent: () => Promise.resolve(() => {}),
  },
}));

const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function peerIdOf(seed: string): string {
  let out = "3xY9";
  for (let i = 0; i < 40; i += 1) {
    out += B58[(seed.charCodeAt(i % seed.length) + i) % B58.length];
  }
  return out;
}

const AGENT_ENDPOINT = {
  wsUrl: "ws://127.0.0.1:8787",
  token: "t",
  peer: "agent-peer",
  endpointId: "ep-1",
  alias: "助手甲",
};

function friendOf(id: string, nickname: string): ChatFriendJson {
  return { peerId: id, nickname, addrs: [], note: null };
}

function friendInvite(peerId: string, nickname: string): FriendInviteJson {
  return {
    peerId, nickname, addrs: [], note: null,
    direction: "out", tsMs: 111, delivered: true,
  };
}

function groupInvite(state: GroupInviteJson["state"]): GroupInviteJson {
  return {
    id: "gi-1", groupId: "g-1", groupName: "项目组", owner: "o", inviter: "o",
    invitee: "i", note: null, direction: "out", state, tsMs: 222, delivered: true,
  };
}

function LocationProbe({ onLocation }: { onLocation: (loc: string) => void }) {
  const location = useLocation();
  useEffect(() => {
    onLocation(location.pathname + location.search);
  }, [location, onLocation]);
  return null;
}

function renderChat(onLocation: (loc: string) => void): void {
  render(
    <MemoryRouter initialEntries={["/chat"]}>
      <ChatPage />
      <LocationProbe onLocation={onLocation} />
    </MemoryRouter>,
  );
}

const noop = () => {};

beforeEach(() => {
  for (const fn of Object.values(mocks)) fn.mockReset();
  mocks.friends.mockResolvedValue([]);
  mocks.history.mockResolvedValue([]);
  mocks.invites.mockResolvedValue([]);
  mocks.groupInvites.mockResolvedValue([]);
  useChatStore.setState({
    friends: [],
    friendsLoaded: true,
    friendsError: null,
    invites: [],
    groupInvites: [],
    groupInvitesLoaded: true,
    groupInvitesError: null,
    selectedPeer: null,
    messagesByPeer: {},
    lastMessageByPeer: {},
    unreadByPeer: {},
    historyLoading: {},
    historyLoaded: {},
    hasMore: {},
    historyError: {},
    olderError: {},
  });
  useGroupStore.setState({
    groups: [],
    groupsLoaded: true,
    groupsError: null,
    friends: [],
    friendsLoaded: true,
    selfPeerId: null,
    selectedGroupId: null,
    messagesByGroup: {},
    lastMessageByGroup: {},
    unreadByGroup: {},
  });
  useAcpStore.setState({
    saved: [AGENT_ENDPOINT],
    phase: "idle",
    activePeer: null,
    activeEndpointId: null,
    focusedEndpointId: null,
    unreadByEndpoint: {},
    lastInteractionByEndpoint: {},
  });
});

describe("F01 新用户空态双 CTA", () => {
  it("列表只有本机 agent：空态文案贴合真实状态，双 CTA 可点通", async () => {
    let loc = "";
    renderChat((next) => (loc = next));
    expect(await screen.findByText("还没有可聊的会话")).toBeTruthy();
    expect(screen.getByText(/会话列表目前只有本机 Agent/)).toBeTruthy();
    fireEvent.click(screen.getByTestId("chat-empty-add-friend"));
    await waitFor(() => expect(loc).toBe("/contacts?add="));
    fireEvent.click(screen.getByTestId("chat-empty-go-contacts"));
    await waitFor(() => expect(loc).toBe("/contacts"));
  });

  it("列表全空：空态给全空措辞 + 双 CTA", async () => {
    useAcpStore.setState({ saved: [] });
    renderChat(noop);
    expect(await screen.findByText("还没有可聊的会话")).toBeTruthy();
    expect(screen.getByText(/会话列表还是空的/)).toBeTruthy();
    expect(screen.getByTestId("chat-empty-add-friend")).toBeTruthy();
    expect(screen.getByTestId("chat-empty-go-contacts")).toBeTruthy();
  });

  it("已有好友/群可选条目：维持原文案，不再出现首公里 CTA", async () => {
    const friend = friendOf(peerIdOf("f1"), "小圆");
    mocks.friends.mockResolvedValue([friend]);
    useChatStore.setState({ friends: [friend] });
    renderChat(noop);
    expect(await screen.findByText("选择或发起会话")).toBeTruthy();
    expect(screen.queryByTestId("chat-empty-add-friend")).toBeNull();
    expect(screen.queryByTestId("chat-empty-go-contacts")).toBeNull();
  });
});

describe("F17 等待对方同意占位", () => {
  it("发出好友邀请后列表出现置灰占位，空态文案切到邀请中", async () => {
    const peer = peerIdOf("invitee");
    const invite = friendInvite(peer, "小待");
    mocks.invites.mockResolvedValue([invite]);
    useChatStore.setState({ invites: [invite] });
    renderChat(noop);
    const row = await screen.findByTestId("conversation-invite-friend-" + peer);
    expect(row.textContent).toContain("小待");
    expect(row.textContent).toContain("等待对方同意");
    expect(row.getAttribute("aria-disabled")).toBe("true");
    expect(screen.getByText(/邀请已发出，等待对方同意/)).toBeTruthy();
    // 占位不产生可选会话：点击不落选中路由（无 button 语义）
    expect(row.querySelector("button")).toBeNull();
  });

  it("群邀请 out+pending 显示占位；终态（rejected）即消失", async () => {
    mocks.groupInvites.mockResolvedValue([groupInvite("pending")]);
    useChatStore.setState({ groupInvites: [groupInvite("pending")] });
    renderChat(noop);
    expect(await screen.findByTestId("conversation-invite-group-g-1")).toBeTruthy();
    useChatStore.setState({ groupInvites: [groupInvite("rejected")] });
    await waitFor(() =>
      expect(screen.queryByTestId("conversation-invite-group-g-1")).toBeNull(),
    );
  });

  it("空列表且无占位：维持「暂无好友」列表空态", async () => {
    useAcpStore.setState({ saved: [] });
    renderChat(noop);
    expect(await screen.findByText("暂无好友")).toBeTruthy();
  });
});

describe("pendingInviteItems 纯函数投影", () => {
  it("只取 out 向（群还须 pending），按发起时间降序", () => {
    const items = pendingInviteItems(
      [friendInvite("a-out", "甲"), { ...friendInvite("a-in", "乙"), direction: "in" }],
      [groupInvite("pending"), { ...groupInvite("rejected"), id: "gi-2" }],
    );
    expect(items.map((i) => i.id + ":" + i.kind)).toEqual(["g-1:group", "a-out:friend"]);
  });

  it("nickname 缺省回退 peerId 缩略", () => {
    const items = pendingInviteItems([friendInvite(peerIdOf("raw"), "")], []);
    expect(items[0]!.title).toBe(peerIdOf("raw").slice(0, 8));
  });
});
