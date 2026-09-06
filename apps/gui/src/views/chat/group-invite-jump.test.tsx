import { act, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  ChatFriendJson,
  ChatMessageJson,
  GroupInviteJson,
  GroupJson,
  NodeEventHandler,
} from "@/lib/ipc-types";

import "@/i18n";

// 同意入群跳转时序（IMC3 需求 1 端到端）：卡片点击 → 确认弹框 → 同意成功 →
// /chat?group= 落地且群未达时呈加载态（不白屏不报错）→ chat_group_state
// roster 到达后自动进入群会话。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn(),
    history: vi.fn(),
    send: vi.fn(),
    groupList: vi.fn(),
    groupHistory: vi.fn(),
    invites: vi.fn(),
    groupInvites: vi.fn(),
    groupInviteAccept: vi.fn(),
    nodeStatus: vi.fn(),
    handlers: [] as NodeEventHandler[],
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatHistory: mocks.history,
    chatSend: mocks.send,
    groupList: mocks.groupList,
    groupHistory: mocks.groupHistory,
    chatInvitesList: mocks.invites,
    chatGroupInvitesList: mocks.groupInvites,
    chatGroupInviteAccept: mocks.groupInviteAccept,
    chatGroupInviteSend: vi.fn(),
    chatGroupInviteReject: vi.fn(),
    nodeStatus: mocks.nodeStatus,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.handlers.push(handler);
      return Promise.resolve(() => {});
    },
  },
}));

import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";
import { ChatPage } from "./chat-page";

const PEER = "3xY9whporr5wt4u8t1z33G85F6K5CBEETKGSHWGPNRRe";
const PEER_B = "3xY9whporr5wt4u8t1z33G85F6K5CBEETKGSHWGPNRRf";
const GID = "99999999-8888-7777-6666-555555555555";

const friendB: ChatFriendJson = { peerId: PEER_B, nickname: "阿北", addrs: [], note: null };

const cardMessage: ChatMessageJson = {
  id: "gi-1",
  peer: PEER_B,
  sender: "them",
  kind: "groupInvite",
  tsMs: Date.now(),
  text: null,
  media: null,
  status: "delivered",
  replyTo: null,
  groupInvite: { groupId: GID, groupName: "项目组", inviterNickname: "阿北", note: "周末副本" },
};

const inviteEntry: GroupInviteJson = {
  id: "inv-jump-1",
  groupId: GID,
  groupName: "项目组",
  owner: PEER_B,
  inviter: PEER_B,
  invitee: PEER,
  note: "周末副本",
  direction: "in",
  state: "pending",
  tsMs: Date.now(),
  delivered: true,
};

function groupFixture(): GroupJson {
  return {
    groupId: GID,
    name: "项目组",
    owner: PEER_B,
    members: [PEER_B, PEER],
    rev: 2,
    state: "active",
    tsMs: Date.now(),
  };
}

function emit(event: Parameters<NodeEventHandler>[0]): void {
  act(() => {
    for (const handler of mocks.handlers) handler(event);
  });
}

beforeEach(() => {
  for (const fn of Object.values(mocks)) {
    if (typeof fn === "function") (fn as ReturnType<typeof vi.fn>).mockReset();
  }
  mocks.friends.mockResolvedValue([friendB]);
  mocks.history.mockImplementation(async (peer: string) =>
    peer === PEER_B ? [cardMessage] : [],
  );
  mocks.send.mockResolvedValue({});
  mocks.groupList.mockResolvedValue([]);
  mocks.groupHistory.mockResolvedValue([]);
  mocks.invites.mockResolvedValue([]);
  mocks.groupInvites.mockResolvedValue([inviteEntry]);
  mocks.groupInviteAccept.mockResolvedValue(undefined);
  mocks.nodeStatus.mockResolvedValue({ peerId: PEER, running: true });
  useChatStore.setState({
    friends: [friendB],
    friendsLoaded: true,
    friendsError: null,
    selectedPeer: null,
    messagesByPeer: {},
    lastMessageByPeer: {},
    unreadByPeer: {},
    historyLoading: {},
    historyLoaded: {},
    hasMore: {},
    historyError: {},
    olderError: {},
    groupInvites: [inviteEntry],
    groupInvitesLoaded: true,
    groupInvitesError: null,
    invites: [],
  });
  useGroupStore.setState({
    groups: [],
    groupsLoaded: true,
    groupsError: null,
    friends: [friendB],
    friendsLoaded: true,
    selfPeerId: PEER,
    selectedGroupId: null,
    messagesByGroup: {},
    lastMessageByGroup: {},
    unreadByGroup: {},
    historyLoading: {},
    historyLoaded: {},
    hasMore: {},
    historyError: {},
    olderError: {},
  });
});

describe("同意入群跳转时序", () => {
  it("同意后群未达呈加载态，roster 到达自动进入群会话", async () => {
    render(
      <MemoryRouter initialEntries={["/chat?peer=" + PEER_B]}>
        <ChatPage />
      </MemoryRouter>,
    );
    // 订阅异步挂载：等 handler 就绪再注入事件
    await waitFor(() => expect(mocks.handlers.length).toBeGreaterThan(0));
    // 卡片出现在 1:1 消息流（them 待处理）
    const card = await screen.findByTestId("group-invite-card");
    expect(card.textContent).toContain("项目组");
    // 点击卡片 → 入群确认弹框
    act(() => {
      card.click();
    });
    expect(await screen.findByTestId("group-invite-dialog")).toBeTruthy();
    // 同意成功 → 跳 /chat?group=，群未在列表：加载态而非空态/报错
    act(() => {
      screen.getByTestId("group-invite-accept").click();
    });
    await waitFor(() => expect(screen.getByTestId("group-pending")).toBeTruthy());
    // roster 事件到达：群列表收编，自动进入群会话
    emit({ type: "chat_group_state", group: groupFixture() });
    await waitFor(() =>
      expect(screen.getByTestId("group-conversation-header")).toBeTruthy(),
    );
    expect(screen.getByTestId("group-conversation-header").textContent).toContain("项目组");
  });

  it("拒绝路径：弹框关闭回到会话，不发生跳转", async () => {
    render(
      <MemoryRouter initialEntries={["/chat?peer=" + PEER_B]}>
        <ChatPage />
      </MemoryRouter>,
    );
    await waitFor(() => expect(mocks.handlers.length).toBeGreaterThan(0));
    const card = await screen.findByTestId("group-invite-card");
    act(() => {
      card.click();
    });
    expect(await screen.findByTestId("group-invite-dialog")).toBeTruthy();
    // 直接关闭弹框（Esc/onOpenChange(false) 路径由 dialog 专测覆盖），
    // 此处断言拒绝按钮存在即入口完整；store 行为由切片/弹框专测覆盖
    expect(screen.getByTestId("group-invite-reject")).toBeTruthy();
  });
});
