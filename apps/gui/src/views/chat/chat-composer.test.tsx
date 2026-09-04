import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ChatFriendJson,
  ChatMessageJson,
  ChatSendReport,
  NodeEventHandler,
} from "@/lib/ipc-types";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    history: vi.fn<() => Promise<ChatMessageJson[]>>(),
    send: vi.fn<(peer: string, kind: string, text?: string, media?: unknown) => Promise<ChatSendReport>>(),
    // 群/1:1 两个 store 各注册一个监听（真实 ipc 事件总线一对多）
    handlers: [] as NodeEventHandler[],
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatHistory: mocks.history,
    chatSend: mocks.send,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.handlers.push(handler);
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

const PEER_A = peerId("friend-a");

function friend(seed: string, nickname: string): ChatFriendJson {
  return { peerId: peerId(seed), nickname, addrs: [], note: null };
}

function msg(
  id: string,
  sender: "me" | "them",
  text: string,
  tsMs: number,
  status: ChatMessageJson["status"] = "delivered",
): ChatMessageJson {
  return { id, peer: PEER_A, sender, kind: "text", tsMs, text, media: null, status };
}

function emitStatus(messageId: string, status: ChatMessageJson["status"]): void {
  act(() => {
    for (const handler of mocks.handlers) {
    handler({ type: "chat_status", peer: PEER_A, messageId, status });
  }
  });
}

async function mountWithHistory(history: ChatMessageJson[]): Promise<void> {
  mocks.friends.mockResolvedValue([friend("friend-a", "小圆")]);
  mocks.history.mockResolvedValue(history);
  render(<ChatView />);
  await waitFor(() => expect(screen.getByText("小圆")).toBeTruthy());
  fireEvent.click(screen.getByText("小圆"));
  await waitFor(() => expect(screen.getByTestId("chat-input")).toBeTruthy());
}

beforeEach(() => {
  mocks.friends.mockReset();
  mocks.history.mockReset();
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

describe("ChatView 发送与状态", () => {
  it("发送文本：回车触发 chatSend，返回消息上屏并显示状态", async () => {
    mocks.send.mockResolvedValue({
      delivered: true,
      message: msg("sent-1", "me", "你好呀", Date.now(), "delivered"),
    });
    await mountWithHistory([]);

    const input = screen.getByTestId("chat-input") as HTMLTextAreaElement;
    fireEvent.change(input, { target: { value: "你好呀" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(mocks.send).toHaveBeenCalledWith(PEER_A, "text", "你好呀");
    });
    await waitFor(() => {
      expect(screen.getAllByText("你好呀").length).toBeGreaterThan(0);
    });
    expect(screen.getAllByText("已送达").length).toBeGreaterThan(0);
  });

  it("状态角标渲染 pending/failed 文字", async () => {
    useChatStore.setState({
      friends: [friend("friend-a", "小圆")],
      friendsLoaded: true,
      selectedPeer: PEER_A,
      messagesByPeer: {
        [PEER_A]: [
          msg("p1", "me", "排队中", Date.now(), "pending"),
          msg("f1", "me", "失败了", Date.now() + 1, "failed"),
          msg("d1", "me", "送达了", Date.now() + 2, "delivered"),
        ],
      },
      lastMessageByPeer: { [PEER_A]: msg("d1", "me", "送达了", Date.now() + 2, "delivered") },
      historyLoading: {},
      hasMore: { [PEER_A]: false },
    });
    render(<ChatView />);
    expect(screen.getByText("排队中")).toBeTruthy();
    expect(screen.getByText("等待对方上线")).toBeTruthy();
    expect(screen.getByText("失败了")).toBeTruthy();
    expect(screen.getByText("失败")).toBeTruthy();
    expect(screen.getAllByText("送达了").length).toBeGreaterThan(0);
    expect(screen.getAllByText("已送达").length).toBeGreaterThan(0);
  });

  it("chat_status 事件推进消息状态 pending→sent→delivered", async () => {
    useChatStore.setState({
      friends: [friend("friend-a", "小圆")],
      friendsLoaded: true,
      selectedPeer: PEER_A,
      messagesByPeer: { [PEER_A]: [msg("s1", "me", "在途", Date.now(), "pending")] },
      lastMessageByPeer: { [PEER_A]: msg("s1", "me", "在途", Date.now(), "pending") },
      historyLoading: {},
      hasMore: { [PEER_A]: false },
    });
    render(<ChatView />);
    expect(screen.getByText("等待对方上线")).toBeTruthy();

    emitStatus("s1", "sent");
    await waitFor(() => expect(screen.getByText("已发送")).toBeTruthy());
    emitStatus("s1", "delivered");
    await waitFor(() => expect(screen.getByText("已送达")).toBeTruthy());
  });
});

describe("ChatView 表情与附件", () => {
  it("打开表情面板选择 emoji 插入输入框光标处", async () => {
    await mountWithHistory([]);

    fireEvent.click(screen.getByRole("button", { name: "表情" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "😀" }));

    const input = screen.getByTestId("chat-input") as HTMLTextAreaElement;
    expect(input.value).toContain("😀");
  });

  it("选择附件走 chatSend(media)，占位气泡后替换为真实消息", async () => {
    mocks.send.mockResolvedValue({
      delivered: true,
      message: {
        id: "media-1",
        peer: PEER_A,
        sender: "me",
        kind: "image",
        tsMs: Date.now(),
        text: null,
        media: { name: "photo.png", mime: "image/png", size: 1200, path: "<app-data>/chat/media/a.png" },
        status: "delivered",
      },
    });
    await mountWithHistory([]);

    const file = new File(["png-bytes"], "photo.png", { type: "image/png" });
    const input = screen.getByTestId("chat-file-input") as HTMLInputElement;
    fireEvent.change(input, { target: { files: [file] } });

    await waitFor(() => {
      expect(mocks.send).toHaveBeenCalledWith(
        PEER_A,
        "image",
        undefined,
        expect.objectContaining({ name: "photo.png", mime: "image/png" }),
      );
    });
    await waitFor(() => {
      expect(screen.getAllByText("photo.png").length).toBeGreaterThan(0);
    });
    expect(screen.getAllByText("已送达").length).toBeGreaterThan(0);
  });
});