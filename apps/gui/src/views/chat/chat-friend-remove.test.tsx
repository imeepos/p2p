import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson, NodeEventHandler } from "@/lib/ipc-types";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    invites: vi.fn(async () => []),
    acceptInvite: vi.fn(async () => null),
    rejectInvite: vi.fn(async () => undefined),
    cancelInvite: vi.fn(async () => true),
    addFriend: vi.fn<
      (peerId: string, nickname: string, addrs: string[]) => Promise<ChatFriendJson>
    >(),
    removeFriend: vi.fn<(peerId: string) => Promise<boolean>>(),
    history: vi.fn<
      (peer: string, beforeId?: string | null, limit?: number) => Promise<unknown[]>
    >(),
    send: vi.fn(),
    eventHandler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatFriendInvite: mocks.addFriend,
    chatInvitesList: mocks.invites,
    chatInviteAccept: mocks.acceptInvite,
    chatInviteReject: mocks.rejectInvite,
    chatInviteCancel: mocks.cancelInvite,
    chatFriendRemove: mocks.removeFriend,
    chatHistory: mocks.history,
    chatSend: mocks.send,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.eventHandler.current = handler;
      return Promise.resolve(() => {});
    },
  },
}));

import "@/i18n";
import { useChatStore } from "@/stores/chat-store";
import { ChatView } from "./chat-view";

// 真实 base58（解码恰 32 字节），与后端 parse_peer_id 同口径的合法夹具
const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";

function friendOf(peerId: string, nickname: string): ChatFriendJson {
  return { peerId, nickname, addrs: [], note: null };
}

beforeEach(() => {
  mocks.friends.mockReset().mockResolvedValue([]);
  mocks.addFriend.mockReset();
  mocks.removeFriend.mockReset().mockResolvedValue(true);
  mocks.history.mockReset().mockResolvedValue([]);
  mocks.send.mockReset();
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
  });
});

async function openRemoveDialog() {
  fireEvent.click(screen.getByTestId(`chat-remove-friend-${PEER}`));
  await waitFor(() => expect(screen.getByTestId("friend-remove-dialog")).toBeTruthy());
}

async function addFriendViaDialog(peerId: string, nickname: string) {
  fireEvent.click(screen.getByTestId("chat-add-friend-empty"));
  await waitFor(() => expect(screen.getByTestId("friend-add-dialog")).toBeTruthy());
  fireEvent.change(screen.getByLabelText("PeerId"), { target: { value: peerId } });
  fireEvent.change(screen.getByLabelText("昵称（可选）"), {
    target: { value: nickname },
  });
  fireEvent.click(screen.getByTestId("friend-add-submit"));
  await waitFor(() =>
    expect(screen.getByTestId(`chat-remove-friend-${peerId}`)).toBeTruthy(),
  );
}

describe("ChatView 移除好友旅程", () => {
  it("移除旅程：添加好友→移除→回到零好友空态→同一 PeerId 再次添加成功", async () => {
    mocks.friends.mockResolvedValueOnce([]);
    mocks.friends.mockResolvedValue([friendOf(PEER, "小圆")]);
    mocks.addFriend.mockImplementation(async (peerId, nickname) =>
      friendOf(peerId, nickname),
    );

    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("暂无好友")).toBeTruthy());

    // 添加好友（邀请语义：同意前好友簿由 mock 数据面呈现）
    await addFriendViaDialog(PEER, "小圆");
    expect(mocks.addFriend).toHaveBeenCalledWith(PEER, "小圆", []);
    await waitFor(() => expect(screen.getByText("小圆")).toBeTruthy());

    // 移除：确认框确认后命令恰好一次，列表即时更新
    await openRemoveDialog();
    fireEvent.click(screen.getByTestId("friend-remove-confirm"));
    await waitFor(() => expect(mocks.removeFriend).toHaveBeenCalledTimes(1));
    expect(mocks.removeFriend).toHaveBeenCalledWith(PEER);
    // 回到零好友空态
    await waitFor(() => expect(screen.getByText("暂无好友")).toBeTruthy());
    // 右侧回未选中空态，不再显示旧会话内容
    expect(screen.queryByTestId("chat-input")).toBeNull();
    expect(
      screen.getByText("暂无会话，选择好友开始聊天"),
    ).toBeTruthy();

    // 同一 PeerId 再次添加成功（回加幂等）
    await addFriendViaDialog(PEER, "小圆");
    expect(mocks.addFriend).toHaveBeenNthCalledWith(2, PEER, "小圆", []);
    await waitFor(() =>
      expect(screen.getAllByText("小圆").length).toBeGreaterThan(0),
    );
  });
});

describe("ChatView 移除确认框拦截", () => {
  it("确认框默认焦点在取消按钮", async () => {
    mocks.friends.mockResolvedValue([friendOf(PEER, "小圆")]);
    render(<ChatView />);
    await waitFor(() => expect(screen.getByTestId(`chat-remove-friend-${PEER}`)).toBeTruthy());
    await openRemoveDialog();
    expect(document.activeElement).toBe(screen.getByTestId("friend-remove-cancel"));
  });

  it("取消与关闭不触发移除命令，确认后移除命令恰好调用一次", async () => {
    mocks.friends.mockResolvedValue([friendOf(PEER, "小圆")]);
    render(<ChatView />);
    await waitFor(() => expect(screen.getByTestId(`chat-remove-friend-${PEER}`)).toBeTruthy());

    // 取消：不触发
    await openRemoveDialog();
    fireEvent.click(screen.getByTestId("friend-remove-cancel"));
    await waitFor(() => expect(screen.queryByTestId("friend-remove-dialog")).toBeNull());
    expect(mocks.removeFriend).not.toHaveBeenCalled();

    // 关闭（Escape）：不触发
    await openRemoveDialog();
    fireEvent.keyDown(screen.getByTestId("friend-remove-dialog"), { key: "Escape" });
    await waitFor(() => expect(screen.queryByTestId("friend-remove-dialog")).toBeNull());
    expect(mocks.removeFriend).not.toHaveBeenCalled();

    // 确认：恰好一次
    await openRemoveDialog();
    fireEvent.click(screen.getByTestId("friend-remove-confirm"));
    await waitFor(() => expect(mocks.removeFriend).toHaveBeenCalledTimes(1));
  });
});

describe("ChatView 选中会话被移除", () => {
  it("选中会话被移除：右侧回空态、选中态清空", async () => {
    mocks.friends.mockResolvedValue([friendOf(PEER, "小圆")]);
    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("小圆")).toBeTruthy());
    fireEvent.click(screen.getByText("小圆"));
    await waitFor(() => expect(screen.getByTestId("chat-input")).toBeTruthy());

    await openRemoveDialog();
    fireEvent.click(screen.getByTestId("friend-remove-confirm"));
    await waitFor(() =>
      expect(screen.getByText("暂无会话，选择好友开始聊天")).toBeTruthy(),
    );
    expect(screen.queryByTestId("chat-input")).toBeNull();
    expect(useChatStore.getState().selectedPeer).toBeNull();
    expect(screen.queryByText("小圆")).toBeNull();
  });
});

describe("ChatView 移除失败路径", () => {
  it("后端拒绝：列表不变、错误可见、不白屏、失败留日志", async () => {
    mocks.friends.mockResolvedValue([friendOf(PEER, "小圆")]);
    mocks.removeFriend.mockRejectedValue(new Error(`后端拒绝：${PEER}`));
    const logSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("小圆")).toBeTruthy());

    await openRemoveDialog();
    fireEvent.click(screen.getByTestId("friend-remove-confirm"));
    await waitFor(() =>
      expect(screen.getByTestId("friend-remove-error").textContent).toContain(
        `后端拒绝：${PEER}`,
      ),
    );
    // 不白屏：确认框保留可重试；列表保持原状
    expect(screen.getByTestId("friend-remove-dialog")).toBeTruthy();
    expect(screen.getByText("小圆")).toBeTruthy();
    expect(logSpy).toHaveBeenCalledWith("[chat] 移除好友失败", PEER, expect.any(Error));
    logSpy.mockRestore();
  });
});
