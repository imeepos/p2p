import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson, ChatMessageJson, NodeEventHandler } from "@/lib/ipc-types";
import { useChatStore } from "@/stores/chat-store";
import { peerId, textMessage } from "@/test/chat-boundaries-fixtures";

// IM-T53 回归：chat_message 事件早于异步历史加载到达时，store 按 id 去重不失效。
// 负载型假红（期望 2 实得 6）根因是超时级联污染 DOM，非 store 缺陷；
// 本文件在 store 层锁死时序语义，防止回归方向修错。
const { mocks } = vi.hoisted(() => ({
  mocks: {
    history: vi.fn<() => Promise<ChatMessageJson[]>>(),
    handler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: vi.fn<() => Promise<ChatFriendJson[]>>(),
    chatHistory: mocks.history,
    chatSend: vi.fn(),
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.handler.current = handler;
      return Promise.resolve(() => {});
    },
  },
}));

const PEER = peerId("store-race");

function emit(event: Parameters<NodeEventHandler>[0]): void {
  // store 事件回调不经 React，无需 act 包裹
  mocks.handler.current?.(event);
}

beforeEach(() => {
  vi.clearAllMocks();
  useChatStore.setState({
    friends: [],
    friendsLoaded: false,
    friendsError: null,
    selectedPeer: null,
    messagesByPeer: {},
    lastMessageByPeer: {},
    historyLoading: {},
    historyLoaded: {},
    hasMore: {},
    historyError: {},
    olderError: {},
  });
});

describe("chat store event/history race", () => {
  it("deduplicates events arriving before history load completes", async () => {
    let resolveHistory!: (page: ChatMessageJson[]) => void;
    mocks.history.mockImplementation(
      () => new Promise<ChatMessageJson[]>((resolve) => { resolveHistory = resolve; }),
    );

    await useChatStore.getState().subscribeEvents();
    const selecting = useChatStore.getState().selectPeer(PEER);

    // 历史在途时事件先到：同 id 事件投递两次
    emit({ type: "chat_message", peer: PEER, message: textMessage("m2", PEER, "早到", { sender: "them" }) });
    emit({ type: "chat_message", peer: PEER, message: textMessage("m2", PEER, "早到", { sender: "them" }) });

    expect(useChatStore.getState().messagesByPeer[PEER].map((m) => m.id)).toEqual(["m2"]);

    resolveHistory([textMessage("m1", PEER, "历史"), textMessage("m2", PEER, "早到", { sender: "them" })]);
    await selecting;

    const state = useChatStore.getState();
    expect(state.historyLoaded[PEER]).toBe(true);
    // 历史页与事件流合并后仍无重复：去重键 = 消息 id
    expect(state.messagesByPeer[PEER].map((m) => m.id)).toEqual(["m1", "m2"]);
    expect(state.lastMessageByPeer[PEER]?.id).toBe("m2");
  });

  it("ignores a late duplicate event after history already merged it", async () => {
    mocks.history.mockResolvedValue([
      textMessage("m1", PEER, "历史"),
      textMessage("m2", PEER, "已并入", { sender: "them" }),
    ]);

    await useChatStore.getState().subscribeEvents();
    await useChatStore.getState().selectPeer(PEER);
    emit({ type: "chat_message", peer: PEER, message: textMessage("m2", PEER, "已并入", { sender: "them" }) });

    const state = useChatStore.getState();
    expect(state.messagesByPeer[PEER].map((m) => m.id)).toEqual(["m1", "m2"]);
    expect(state.lastMessageByPeer[PEER]?.id).toBe("m2");
  });
});
