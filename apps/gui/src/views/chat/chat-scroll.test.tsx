import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ChatFriendJson,
  ChatMessageJson,
  NodeEventHandler,
} from "@/lib/ipc-types";
import { ChatPage } from "./chat-page";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    history: vi.fn<
      (peer: string, beforeId?: string | null, limit?: number) => Promise<ChatMessageJson[]>
    >(),
    send: vi.fn(),
    groupList: vi.fn<() => Promise<never[]>>(),
    groupHistory: vi.fn<() => Promise<never[]>>(),
    invites: vi.fn<() => Promise<never[]>>(),
    nodeStatus: vi.fn(),
    eventHandler: { current: null as NodeEventHandler | null },
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
    nodeStatus: mocks.nodeStatus,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.eventHandler.current = handler;
      return Promise.resolve(() => {});
    },
  },
}));

import { useChatStore } from "@/stores/chat-store";
import "@/i18n";

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

// IM-T52 回归（P1 双栏结构）：jsdom 无布局引擎，高度链的数值断言由无头
// Chrome 度量；这里固化滚动体系的 DOM 结构契约，防止布局类回归
// （min-h 魔法数、嵌套滚动域、输入条漂移）。
describe("IM-T52 滚动体系结构契约（P1 双栏）", () => {
  const a = friend("friend-a", "小圆");
  const b = friend("friend-b", "阿圆");

  beforeEach(() => {
    mocks.friends.mockReset().mockResolvedValue([a, b]);
    mocks.history.mockReset().mockResolvedValue([]);
    mocks.groupList.mockReset().mockResolvedValue([]);
    mocks.groupHistory.mockReset().mockResolvedValue([]);
    mocks.invites.mockReset().mockResolvedValue([]);
    mocks.nodeStatus.mockReset().mockResolvedValue({ peerId: "self", running: true });
    mocks.send.mockReset();
    useChatStore.setState({
      friends: [],
      friendsLoaded: false,
      friendsError: null,
      selectedPeer: null,
      messagesByPeer: {},
      lastMessageByPeer: {},
      unreadByPeer: {},
      historyLoading: {},
      historyLoaded: {},
      hasMore: {},
    });
  });

  const rowButton = (peer: ChatFriendJson) =>
    screen.getByTestId("conversation-row-friend-" + peer.peerId);

  async function renderChat() {
    render(
      <MemoryRouter initialEntries={["/chat"]}>
        <ChatPage />
      </MemoryRouter>,
    );
    await waitFor(() =>
      expect(screen.getByTestId("conversation-row-friend-" + a.peerId)).toBeTruthy(),
    );
  }

  it("聊天页容器精确填充：flex-1 + min-h-0，禁止 100vh 魔法数回归", async () => {
    await renderChat();
    const page = screen.getByTestId("chat-page");
    expect(page.className).toContain("flex-1");
    expect(page.className).toContain("min-h-0");
    expect(page.className).not.toContain("min-h-[calc");
    expect(page.className).not.toContain("100vh");
  });

  it("滚动域分离：会话列表与消息列表各自内滚且互不嵌套", async () => {
    await renderChat();
    fireEvent.click(rowButton(a));
    await waitFor(() => expect(screen.getByTestId("message-scroll")).toBeTruthy());
    const list = screen.getByTestId("conversation-items");
    const messages = screen.getByTestId("message-scroll");
    for (const el of [list, messages]) {
      expect(el.className).toContain("overflow-y-auto");
      expect(el.className).toContain("min-h-0");
      expect(el.className).toContain("scroll-slim");
    }
    expect(list.contains(messages)).toBe(false);
    expect(messages.contains(list)).toBe(false);
  });

  it("消息流横向滚动零容忍：滚动域 overflow-x-hidden，连续消息有纵向间隔", async () => {
    await renderChat();
    fireEvent.click(rowButton(a));
    await waitFor(() => expect(screen.getByTestId("message-scroll")).toBeTruthy());
    const messages = screen.getByTestId("message-scroll");
    expect(messages.className).toContain("overflow-x-hidden");
    expect(screen.getByTestId("message-column").className).toContain("gap-y-2.5");
  });

  it("消息流包装层必须是 flex-col：块级包装会让滚动域高度随内容生长、溢出遮挡输入条", async () => {
    // jsdom 无布局引擎，高度链做数值断言不可行；这里固化结构契约——
    // 滚动域的 flex-1 只有在 flex 格式化上下文中才会被压到剩余空间。
    await renderChat();
    fireEvent.click(rowButton(a));
    await waitFor(() => expect(screen.getByTestId("message-scroll")).toBeTruthy());
    const wrapper = screen.getByTestId("message-scroll").parentElement;
    expect(wrapper).toBeTruthy();
    expect(wrapper!.className).toContain("flex");
    expect(wrapper!.className).toContain("flex-col");
    expect(wrapper!.className).toContain("min-h-0");
  });

  it("输入条钉在消息滚动域之外：DOM 序上位于消息列表之后", async () => {
    await renderChat();
    fireEvent.click(rowButton(a));
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

  it("列表滚动位置独立保持：切走再切回不丢", async () => {
    await renderChat();
    fireEvent.click(rowButton(a));
    await waitFor(() => expect(screen.getByTestId("chat-input")).toBeTruthy());

    const list = screen.getByTestId("conversation-items");
    list.scrollTop = 120;
    fireEvent.click(rowButton(b));
    await waitFor(() =>
      expect(mocks.history).toHaveBeenCalledWith(b.peerId, null, 20),
    );
    expect(list.scrollTop).toBe(120);

    fireEvent.click(rowButton(a));
    await waitFor(() =>
      expect(mocks.history).toHaveBeenCalledWith(a.peerId, null, 20),
    );
    expect(list.scrollTop).toBe(120);
  });
});
