import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson, ChatMessageJson, NodeEventHandler } from "@/lib/ipc-types";
import { peerId, textMessage } from "@/test/chat-boundaries-fixtures";
import { useChatStore } from "@/stores/chat-store";

// §2.3 未读计数（1:1）：未选中收消息 +1、选中清零、自己消息不计。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    history: vi.fn<() => Promise<ChatMessageJson[]>>(),
    handler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatHistory: mocks.history,
    chatSend: vi.fn(),
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.handler.current = handler;
      return Promise.resolve(() => {});
    },
  },
}));

const PEER = peerId("unread-peer");
const OTHER = peerId("unread-other");

function emit(event: Parameters<NodeEventHandler>[0]): void {
  mocks.handler.current?.(event);
}

beforeEach(async () => {
  vi.clearAllMocks();
  mocks.friends.mockResolvedValue([]);
  mocks.history.mockResolvedValue([]);
  useChatStore.setState({
    friends: [],
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
  });
  await useChatStore.getState().subscribeEvents();
});

describe("chat-store 未读（§2.3）", () => {
  it("未选中会话收到对方消息 unread+1", () => {
    emit({ type: "chat_message", peer: PEER, message: textMessage("u1", PEER, "来了", { sender: "them" }) });
    expect(useChatStore.getState().unreadByPeer[PEER]).toBe(1);
    emit({ type: "chat_message", peer: PEER, message: textMessage("u2", PEER, "又来", { sender: "them" }) });
    expect(useChatStore.getState().unreadByPeer[PEER]).toBe(2);
  });

  it("自己发出的消息不计未读", () => {
    emit({ type: "chat_message", peer: PEER, message: textMessage("u3", PEER, "自言自语", { sender: "me" }) });
    expect(useChatStore.getState().unreadByPeer[PEER]).toBeUndefined();
  });

  it("选中会话（selectPeer）清零；其他会话计数不受影响", async () => {
    emit({ type: "chat_message", peer: PEER, message: textMessage("u4", PEER, "a", { sender: "them" }) });
    emit({ type: "chat_message", peer: OTHER, message: textMessage("u5", OTHER, "b", { sender: "them" }) });
    expect(useChatStore.getState().unreadByPeer[PEER]).toBe(1);
    await useChatStore.getState().selectPeer(PEER);
    expect(useChatStore.getState().unreadByPeer[PEER]).toBe(0);
    expect(useChatStore.getState().unreadByPeer[OTHER]).toBe(1);
  });

  it("选中期间到达的消息不累积未读", async () => {
    await useChatStore.getState().selectPeer(PEER);
    emit({ type: "chat_message", peer: PEER, message: textMessage("u6", PEER, "即时", { sender: "them" }) });
    expect(useChatStore.getState().unreadByPeer[PEER] ?? 0).toBe(0);
  });
});
