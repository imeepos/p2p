import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ChatFriendJson,
  ChatMessageJson,
  NodeEventJson,
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

import "@/i18n";
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

function msg(
  peer: string,
  id: string,
  sender: "me" | "them",
  text: string,
  tsMs: number,
  status: ChatMessageJson["status"] = "delivered",
): ChatMessageJson {
  return { id, peer, sender, kind: "text", tsMs, text, media: null, status };
}

const PEER_A = peerId("friend-a");

function emit(event: NodeEventJson): void {
  act(() => {
    mocks.eventHandler.current?.(event);
  });
}

beforeEach(() => {
  mocks.friends.mockReset().mockResolvedValue([]);
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

describe("ChatView 会话列表", () => {
  it("渲染好友昵称/缩略 PeerId/最后消息摘要/时间，按最后消息时间倒序", async () => {
    const a = friend("friend-a", "小圆");
    const b = friend("friend-b", "");
    mocks.friends.mockResolvedValue([a, b]);
    mocks.history.mockImplementation(async (peer: string) => {
      if (peer === a.peerId) return [msg(peer, "a1", "them", "晚安", Date.now() - 60000)];
      if (peer === b.peerId) return [msg(peer, "b1", "me", "今天加班", Date.now() - 3600000)];
      return [];
    });
    render(<ChatView />);
    await waitFor(() => expect(mocks.friends).toHaveBeenCalled());

    expect(screen.getByText("小圆")).toBeTruthy();
    expect(screen.getByText("晚安")).toBeTruthy();
    expect(screen.getByText(b.peerId.slice(0, 8))).toBeTruthy();
    expect(screen.getByText("今天加班")).toBeTruthy();
    const items = screen.getAllByRole("button").filter(
      (el) =>
        el.textContent?.includes(a.peerId.slice(0, 12)) ||
        el.textContent?.includes(b.peerId.slice(0, 12)),
    );
    expect(items).toHaveLength(2);
    expect(items[0]!.textContent).toContain("晚安");
    expect(items[1]!.textContent).toContain("今天加班");
  });

  it("无好友时显示空态引导", async () => {
    mocks.friends.mockResolvedValue([]);
    mocks.history.mockResolvedValue([]);
    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("暂无好友")).toBeTruthy());
  });
});

describe("ChatView 历史与事件", () => {
  it("选择好友后加载历史并渲染气泡", async () => {
    mocks.friends.mockResolvedValue([friend("friend-a", "小圆")]);
    mocks.history.mockImplementation(async (peer: string) =>
      peer === PEER_A ? [msg(peer, "a1", "them", "你好", 1000), msg(peer, "a2", "me", "嗨", 2000)] : [],
    );
    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("小圆")).toBeTruthy());

    fireEvent.click(screen.getByText("小圆"));
    await waitFor(() => expect(mocks.history).toHaveBeenCalledWith(PEER_A, null, 50));
    expect(screen.getAllByText("你好").length).toBeGreaterThan(0);
    expect(screen.getAllByText("嗨").length).toBeGreaterThan(0);
  });

  it("chat_message 事件实时插入气泡", async () => {
    mocks.friends.mockResolvedValue([friend("friend-a", "小圆")]);
    mocks.history.mockImplementation(async (peer: string) =>
      peer === PEER_A ? [msg(peer, "a1", "them", "你好", 1000)] : [],
    );
    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("小圆")).toBeTruthy());
    fireEvent.click(screen.getByText("小圆"));
    await waitFor(() => {
      expect(screen.getAllByText("你好").length).toBeGreaterThan(0);
    });

    emit({
      type: "chat_message",
      peer: PEER_A,
      message: msg(PEER_A, "a2", "them", "新消息到了", Date.now()),
    });
    await waitFor(() => {
      expect(screen.getAllByText("新消息到了").length).toBeGreaterThan(0);
    });
  });

  it("分页：滚动到顶部加载更早页（beforeId 游标）", async () => {
    const page1 = Array.from({ length: 50 }, (_, i) =>
      msg(PEER_A, `m${i}`, i % 2 === 0 ? "me" : "them", `消息${i}`, 1000 + i),
    );
    const older = [msg(PEER_A, "old2", "them", "更早2", 500), msg(PEER_A, "old1", "me", "更早1", 400)];
    mocks.friends.mockResolvedValue([friend("friend-a", "小圆")]);
    mocks.history.mockImplementation(async (peer: string, beforeId?: string | null) => {
      if (peer !== PEER_A) return [];
      return beforeId ? older : page1;
    });
    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("小圆")).toBeTruthy());
    fireEvent.click(screen.getByText("小圆"));
    await waitFor(() => {
      expect(screen.getAllByText("消息49").length).toBeGreaterThan(0);
    });

    const scroll = screen.getByTestId("message-scroll");
    Object.defineProperty(scroll, "scrollHeight", { value: 2000, configurable: true });
    Object.defineProperty(scroll, "clientHeight", { value: 400, configurable: true });
    Object.defineProperty(scroll, "scrollTop", { value: 0, configurable: true });
    fireEvent.scroll(scroll);

    await waitFor(() => {
      expect(mocks.history).toHaveBeenCalledWith(PEER_A, page1[0]!.id, 50);
    });
    await waitFor(() => {
      expect(screen.getAllByText("更早1").length).toBeGreaterThan(0);
    });
  });
});

describe("ChatView 三态一致性", () => {
  it("加载中显示好友加载中文案而非未知", async () => {
    mocks.friends.mockImplementation(() => new Promise(() => {}));
    render(<ChatView />);
    expect(screen.getByText("正在加载好友…")).toBeTruthy();
  });

  it("加载失败显示错误原文与刷新入口，重试成功恢复列表", async () => {
    mocks.friends.mockRejectedValueOnce(new Error("friends boom"));
    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("friends boom")).toBeTruthy());
    expect(screen.getByRole("button", { name: "刷新" })).toBeTruthy();
    const logSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    mocks.friends.mockResolvedValue([friend("friend-a", "小圆")]);
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    await waitFor(() => expect(screen.getByText("小圆")).toBeTruthy());
    logSpy.mockRestore();
  });
});
