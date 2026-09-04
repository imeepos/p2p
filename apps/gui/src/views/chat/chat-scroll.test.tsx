import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ChatFriendJson,
  ChatMessageJson,
  NodeEventHandler,
} from "@/lib/ipc-types";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    history: vi.fn<
      (peer: string, beforeId?: string | null, limit?: number) => Promise<ChatMessageJson[]>
    >(),
    send: vi.fn(),
    eventHandler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatHistory: mocks.history,
    chatSend: mocks.send,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.eventHandler.current = handler;
      return Promise.resolve(() => {});
    },
  },
}));

import { useChatStore } from "@/stores/chat-store";
import { ChatView } from "./chat-view";

const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function peerId(seed: string): string {
  let out = "3xY9";
  for (let i = 0; i < 40; i += 1) {
    out += B58[(seed.charCodeAt(i % seed.length) + i) % B58.length];
  }
  return out;
}

function friend(seed: string, nickname: string): ChatFriendJson {
  return { peerId: peerId(seed), nickname, addrs: [], note: null };
}

// IM-T52 回归：jsdom 无布局引擎，高度链的数值断言由无头 Chrome 度量；
// 这里固化滚动体系的 DOM 结构契约，防止布局类回归（min-h 魔法数、嵌套滚动域）。
describe("IM-T52 滚动体系结构契约", () => {
  const a = friend("friend-a", "小圆");
  const b = friend("friend-b", "阿圆");

  beforeEach(() => {
    mocks.friends.mockReset().mockResolvedValue([a, b]);
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

  const rowButton = (peer: ChatFriendJson) =>
    screen
      .getAllByRole("button")
      .filter((el) => el.textContent?.includes(peer.peerId.slice(0, 12)));

  async function renderChat() {
    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("小圆")).toBeTruthy());
  }

  it("聊天栅格精确填充：flex-1 + min-h-0，禁止 100vh 魔法数回归", async () => {
    await renderChat();
    const grid = screen.getByTestId("chat-grid");
    expect(grid.className).toContain("flex-1");
    expect(grid.className).toContain("min-h-0");
    expect(grid.className).not.toContain("min-h-[calc");
    expect(grid.className).not.toContain("100vh");
  });

  it("滚动域分离：好友列表与消息列表各自内滚且互不嵌套", async () => {
    await renderChat();
    fireEvent.click(rowButton(a)[0]!);
    await waitFor(() => expect(screen.getByTestId("message-scroll")).toBeTruthy());
    const friends = screen.getByTestId("friends-scroll");
    const messages = screen.getByTestId("message-scroll");
    for (const el of [friends, messages]) {
      expect(el.className).toContain("overflow-y-auto");
      expect(el.className).toContain("min-h-0");
      expect(el.className).toContain("scroll-slim");
    }
    expect(friends.contains(messages)).toBe(false);
    expect(messages.contains(friends)).toBe(false);
  });

  it("输入条钉在消息滚动域之外：DOM 序上位于消息列表之后", async () => {
    await renderChat();
    fireEvent.click(rowButton(a)[0]!);
    await waitFor(() => expect(screen.getByTestId("chat-input")).toBeTruthy());

    const messages = screen.getByTestId("message-scroll");
    const input = screen.getByTestId("chat-input");
    expect(messages.contains(input)).toBe(false);
    expect(messages.contains(screen.getByTestId("chat-conversation-header"))).toBe(
      false,
    );
    const composerRoot = input.closest("div.shrink-0")!;
    const following = messages.compareDocumentPosition(composerRoot);
    expect(following & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("好友域滚动位置独立保持：切走再切回不丢", async () => {
    await renderChat();
    fireEvent.click(rowButton(a)[0]!);
    await waitFor(() => expect(screen.getByTestId("chat-input")).toBeTruthy());

    const friends = screen.getByTestId("friends-scroll");
    friends.scrollTop = 120;
    fireEvent.click(rowButton(b)[0]!);
    await waitFor(() =>
      expect(mocks.history).toHaveBeenCalledWith(b.peerId, null, 50),
    );
    expect(friends.scrollTop).toBe(120);

    fireEvent.click(rowButton(a)[0]!);
    await waitFor(() =>
      expect(mocks.history).toHaveBeenCalledWith(a.peerId, null, 50),
    );
    expect(friends.scrollTop).toBe(120);
  });
});
