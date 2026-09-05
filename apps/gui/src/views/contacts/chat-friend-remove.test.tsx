import { useState } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson, NodeEventHandler } from "@/lib/ipc-types";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    history: vi.fn<() => Promise<never[]>>(),
    removeFriend: vi.fn<(peerId: string) => Promise<boolean>>(),
    addFriend: vi.fn(),
    send: vi.fn(),
    handlers: [] as NodeEventHandler[],
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatHistory: mocks.history,
    chatFriendRemove: mocks.removeFriend,
    chatFriendAdd: mocks.addFriend,
    chatSend: mocks.send,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.handlers.push(handler);
      return Promise.resolve(() => {});
    },
  },
}));

import "@/i18n";
import { useChatStore } from "@/stores/chat-store";
import { ChatFriendRemoveDialog } from "@/views/contacts/chat-friend-remove-dialog";

// 真实 base58（解码恰 32 字节），与后端 parse_peer_id 同口径的合法夹具
const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";

function friendOf(peerId: string, nickname: string): ChatFriendJson {
  return { peerId, nickname, addrs: [], note: null };
}

// P1 起 /chat 列表不再承载好友管理入口（归 P2 通讯录），本文件直挂对话框
// 组件覆盖确认框交互/失败路径/选中态清零；入口可达性回归随 P2 迁移重建。
// Harness 复刻父组件语义：onOpenChange(false) 即从树上摘除对话框。
function RemoveHarness({ target }: { target: ChatFriendJson }) {
  const [friend, setFriend] = useState<ChatFriendJson | null>(target);
  return (
    <ChatFriendRemoveDialog
      friend={friend}
      onOpenChange={(open) => {
        if (!open) setFriend(null);
      }}
    />
  );
}

function openRemoveDialog(target: ChatFriendJson): void {
  useChatStore.setState({ friends: [target], friendsLoaded: true });
  render(<RemoveHarness target={target} />);
}

beforeEach(() => {
  mocks.friends.mockReset().mockResolvedValue([friendOf(PEER, "小圆")]);
  mocks.history.mockReset().mockResolvedValue([]);
  mocks.removeFriend.mockReset().mockResolvedValue(true);
  mocks.addFriend.mockReset();
  mocks.send.mockReset();
  useChatStore.setState({
    invites: [],
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

describe("ChatFriendRemoveDialog 确认框", () => {
  it("确认框默认焦点在取消按钮", async () => {
    openRemoveDialog(friendOf(PEER, "小圆"));
    await waitFor(() => expect(screen.getByTestId("friend-remove-dialog")).toBeTruthy());
    expect(document.activeElement).toBe(screen.getByTestId("friend-remove-cancel"));
  });

  it("取消与关闭不触发移除命令，确认后移除命令恰好调用一次", async () => {
    openRemoveDialog(friendOf(PEER, "小圆"));
    await waitFor(() => expect(screen.getByTestId("friend-remove-dialog")).toBeTruthy());

    // 取消：不触发
    fireEvent.click(screen.getByTestId("friend-remove-cancel"));
    await waitFor(() => expect(screen.queryByTestId("friend-remove-dialog")).toBeNull());
    expect(mocks.removeFriend).not.toHaveBeenCalled();

    // 关闭（Escape）：不触发
    openRemoveDialog(friendOf(PEER, "小圆"));
    fireEvent.keyDown(screen.getByTestId("friend-remove-dialog"), { key: "Escape" });
    await waitFor(() => expect(screen.queryByTestId("friend-remove-dialog")).toBeNull());
    expect(mocks.removeFriend).not.toHaveBeenCalled();

    // 确认：恰好一次
    openRemoveDialog(friendOf(PEER, "小圆"));
    fireEvent.click(screen.getByTestId("friend-remove-confirm"));
    await waitFor(() => expect(mocks.removeFriend).toHaveBeenCalledTimes(1));
    expect(mocks.removeFriend).toHaveBeenCalledWith(PEER);
  });
});

describe("ChatFriendRemoveDialog 移除与失败路径", () => {
  it("移除成功：store 好友簿与选中态同步清空（选中回空态语义）", async () => {
    useChatStore.setState({
      selectedPeer: PEER,
      messagesByPeer: { [PEER]: [] },
    });
    openRemoveDialog(friendOf(PEER, "小圆"));
    fireEvent.click(screen.getByTestId("friend-remove-confirm"));
    await waitFor(() => expect(mocks.removeFriend).toHaveBeenCalledTimes(1));
    const state = useChatStore.getState();
    expect(state.friends.find((f) => f.peerId === PEER)).toBeUndefined();
    expect(state.selectedPeer).toBeNull();
  });

  it("后端拒绝：错误原文在框内展示、不白屏、失败留日志", async () => {
    mocks.removeFriend.mockRejectedValue(new Error(`后端拒绝：${PEER}`));
    const logSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    openRemoveDialog(friendOf(PEER, "小圆"));
    fireEvent.click(screen.getByTestId("friend-remove-confirm"));
    await waitFor(() =>
      expect(screen.getByTestId("friend-remove-error").textContent).toContain(
        `后端拒绝：${PEER}`,
      ),
    );
    // 不白屏：确认框保留可重试；store 好友簿保持原状
    expect(screen.getByTestId("friend-remove-dialog")).toBeTruthy();
    expect(useChatStore.getState().friends.find((f) => f.peerId === PEER)).toBeTruthy();
    expect(logSpy).toHaveBeenCalledWith("[chat] 移除好友失败", PEER, expect.any(Error));
    logSpy.mockRestore();
  });
});
