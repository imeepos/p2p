import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ChatFriendJson,
  ChatMessageJson,
  ChatSendReport,
  NodeEventHandler,
} from "@/lib/ipc-types";

const { mocks, toastSpies } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    history: vi.fn<() => Promise<ChatMessageJson[]>>(),
    send: vi.fn<(peer: string, kind: string, text?: string, media?: unknown) => Promise<ChatSendReport>>(),
    // 群/1:1 两个 store 各注册一个监听（真实 ipc 事件总线一对多）
    handlers: [] as NodeEventHandler[],
  },
  toastSpies: { error: vi.fn() },
}));

vi.mock("@/components/feedback/toast", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/components/feedback/toast")>();
  return {
    ...actual,
    toastError: (...args: Parameters<typeof actual.toastError>) => {
      toastSpies.error(...args);
      return actual.toastError(...args);
    },
  };
});

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
import { Composer } from "@/components/chat/composer";
import { FriendConversation } from "./friend-conversation";

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
  useChatStore.setState({
    friends: [friend("friend-a", "小圆")],
    friendsLoaded: true,
    selectedPeer: null,
    messagesByPeer: {},
    lastMessageByPeer: {},
    historyLoading: {},
    historyLoaded: {},
    hasMore: {},
  });
  // 历史经真实 selectPeer → mock chatHistory 装载（与生产同路径）；
  // 事件订阅与 ChatPage 挂载面同源（状态推进事件依赖它）。
  await useChatStore.getState().subscribeEvents();
  await act(async () => {
    await useChatStore.getState().selectPeer(PEER_A);
  });
  render(<FriendConversation peer={PEER_A} />);
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
    render(<FriendConversation peer={PEER_A} />);
    expect(screen.getByText("排队中")).toBeTruthy();
    expect(screen.getByText("发送中…")).toBeTruthy();
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
    render(<FriendConversation peer={PEER_A} />);
    expect(screen.getByText("发送中…")).toBeTruthy();

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
    // 选择成功后面板自动收起
    expect(screen.queryByRole("menu")).toBeNull();
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

describe("composer 前置校验（§2.5）", () => {
  it("媒体超 64MiB：读取前本地拦截，不发无效请求，走稳定错误码 i18n 提示", async () => {
    await mountWithHistory([]);
    const file = new File(["x"], "big.png", { type: "image/png" });
    Object.defineProperty(file, "size", { value: 64 * 1024 * 1024 + 1 });
    const input = screen.getByTestId("chat-file-input") as HTMLInputElement;
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    fireEvent.change(input, { target: { files: [file] } });
    await waitFor(() =>
      expect(toastSpies.error).toHaveBeenCalledWith(
        "附件超过单条消息 64 MiB 上限",
        expect.objectContaining({ description: expect.stringContaining("guard=tooLarge") }),
      ),
    );
    expect(mocks.send).not.toHaveBeenCalled();
    expect(input.value).toBe("");
    expect(warnSpy).toHaveBeenCalled();
    warnSpy.mockRestore();
  });

  it("空文件：前置拦截为空载荷错误码，不发无效请求", async () => {
    await mountWithHistory([]);
    const file = new File([], "empty.bin", { type: "application/octet-stream" });
    const input = screen.getByTestId("chat-file-input") as HTMLInputElement;
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    fireEvent.change(input, { target: { files: [file] } });
    await waitFor(() =>
      expect(toastSpies.error).toHaveBeenCalledWith("附件内容为空，无法发送", expect.anything()),
    );
    expect(mocks.send).not.toHaveBeenCalled();
    warnSpy.mockRestore();
  });
});

describe("composer 注入 transport（A2A 会话复用）", () => {
  beforeEach(() => {
    toastSpies.error.mockClear();
  });

  // 2026-09-09 回归：A2A transport resolve undefined（无 ChatSendReport），
  // composer 曾把 undefined 当报告判 delivered 炸 TypeError，误报「发送失败」。
  it("transport 返回非报告值：不误报失败，输入照常清空", async () => {
    const sendText = vi.fn<(peer: string, text: string) => Promise<unknown>>();
    sendText.mockResolvedValue(undefined);
    render(
      <Composer
        peer="agent-key"
        replyTarget={null}
        onReplyCancel={() => {}}
        transport={{ sendText, sendMedia: vi.fn() }}
      />,
    );
    const input = screen.getByTestId("chat-input") as HTMLTextAreaElement;
    fireEvent.change(input, { target: { value: "你好 agent" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(sendText).toHaveBeenCalled());
    await waitFor(() => expect(input.value).toBe(""));
    expect(toastSpies.error).not.toHaveBeenCalled();
  });

  it("transport 拒绝：失败原文 toast（禁静默）", async () => {
    const sendText = vi.fn<(peer: string, text: string) => Promise<unknown>>();
    sendText.mockRejectedValue(new Error("任务通道未配置（console 未连接）"));
    render(
      <Composer
        peer="agent-key"
        replyTarget={null}
        onReplyCancel={() => {}}
        transport={{ sendText, sendMedia: vi.fn() }}
      />,
    );
    const input = screen.getByTestId("chat-input") as HTMLTextAreaElement;
    fireEvent.change(input, { target: { value: "你好 agent" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() =>
      expect(toastSpies.error).toHaveBeenCalledWith(
        "发送失败",
        expect.objectContaining({ description: expect.stringContaining("任务通道未配置") }),
      ),
    );
    expect(input.value).toBe("你好 agent");
  });
});