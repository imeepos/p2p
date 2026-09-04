import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson, ChatMessageJson, ChatSendReport, NodeEventHandler } from "@/lib/ipc-types";
import { MediaContent } from "@/components/chat/media-content";
import { Composer } from "@/components/chat/composer";
import { useChatStore } from "@/stores/chat-store";
import {
  chatMedia,
  friendJson,
  mediaMessage,
  oversizedFile,
  peerId,
  sendReport,
  textMessage,
} from "@/test/chat-boundaries-fixtures";
import "@/i18n";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    history: vi.fn<() => Promise<ChatMessageJson[]>>(),
    send: vi.fn<() => Promise<ChatSendReport>>(),
    handler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatHistory: mocks.history,
    chatSend: mocks.send,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.handler.current = handler;
      return Promise.resolve(() => {});
    },
  },
}));

const PEER = peerId("boundary-a");

// IM-T53 负载加固：重负载下动态 import/渲染可超 vitest 默认 5s，超时用例的
// 后台 continuation 会在 cleanup 之后渲染游离 ChatView 污染后续 DOM 计数
// （「期望 2 实得 6」根因）。逐点给足预算，不全局放宽 testTimeout。
const VIEW_TIMEOUT = 20_000;
const WAIT_TIMEOUT = 10_000;

function resetStore(): void {
  useChatStore.setState({ friends: [], friendsLoaded: false, friendsError: null, selectedPeer: null, messagesByPeer: {}, lastMessageByPeer: {}, historyLoading: {}, historyLoaded: {}, hasMore: {}, historyError: {}, olderError: {} });
}

// 直塞 store 选中会话：绕开 loadFriends/selectPeer，聚焦渲染边界。
function selectConversation(messages: ChatMessageJson[]): void {
  useChatStore.setState({
    friends: [friendJson(PEER)],
    friendsLoaded: true,
    selectedPeer: PEER,
    messagesByPeer: { [PEER]: messages },
    lastMessageByPeer: { [PEER]: messages[messages.length - 1] ?? null },
    historyLoading: {},
    hasMore: { [PEER]: false },
  });
}

function emit(event: Parameters<NodeEventHandler>[0]): void {
  act(() => mocks.handler.current?.(event));
}

async function mountComposer(): Promise<void> {
  selectConversation([]);
  render(<Composer peer={PEER} replyTarget={null} onReplyCancel={() => {}} />);
  await waitFor(() => expect(screen.getByTestId("chat-input")).toBeTruthy(), { timeout: WAIT_TIMEOUT });
}

beforeEach(() => {
  vi.clearAllMocks();
  resetStore();
  mocks.friends.mockResolvedValue([friendJson(PEER)]);
  mocks.history.mockResolvedValue([]);
  mocks.send.mockResolvedValue(sendReport(textMessage("r1", PEER, "回执")));
});

describe("GUI chat composer boundaries", () => {
  it("rejects blank text, accepts unicode and exact 2000, rejects 2001", async () => {
    await mountComposer();
    const input = screen.getByTestId("chat-input") as HTMLTextAreaElement;
    const send = screen.getByTestId("chat-send");
    expect(send).toBeDisabled();
    fireEvent.change(input, { target: { value: "   \n\t" } });
    expect(send).toBeDisabled();
    fireEvent.change(input, { target: { value: "你好😀" } });
    expect(send).toBeEnabled();
    fireEvent.change(input, { target: { value: "x".repeat(2000) } });
    expect(send).toBeEnabled();
    fireEvent.change(input, { target: { value: "x".repeat(2001) } });
    expect(send).toBeDisabled();
    expect(screen.getByText(/超过 2000/)).toBeTruthy();
  }, 15_000);

  it("toggles emoji panel on repeated clicks and inserts at selection", async () => {
    await mountComposer();
    const input = screen.getByTestId("chat-input") as HTMLTextAreaElement;
    fireEvent.change(input, { target: { value: "ab" } });
    input.setSelectionRange(1, 1);
    const toggle = screen.getByRole("button", { name: "表情" });
    fireEvent.click(toggle);
    expect(screen.getByRole("menu")).toBeTruthy();
    fireEvent.click(screen.getByRole("menuitem", { name: "😀" }));
    await waitFor(() => expect(input.value).toBe("a😀b"), { timeout: WAIT_TIMEOUT });
    fireEvent.click(toggle);
    expect(screen.queryByRole("menu")).toBeNull();
    fireEvent.click(toggle);
    expect(screen.getByRole("menu")).toBeTruthy();
  }, 15_000);

  it("sends image/audio/video/file kinds incl. zero-byte and unknown MIME", async () => {
    await mountComposer();
    const input = screen.getByTestId("chat-file-input") as HTMLInputElement;
    const cases: Array<[File, string]> = [
      [new File(["x"], "photo.png", { type: "image/png" }), "image"],
      [new File(["x"], "sound.mp3", { type: "audio/mpeg" }), "audio"],
      [new File(["x"], "clip.mp4", { type: "video/mp4" }), "video"],
      [new File(["x"], "archive.unknown", { type: "application/octet-stream" }), "file"],
      [new File([], "empty.bin", { type: "application/octet-stream" }), "file"],
    ];
    for (const [file, kind] of cases) {
      fireEvent.change(input, { target: { files: [file] } });
      await waitFor(
        () => expect(mocks.send).toHaveBeenCalledWith(PEER, kind, undefined, expect.objectContaining({ name: file.name })),
        { timeout: WAIT_TIMEOUT },
      );
      mocks.send.mockClear();
    }
  }, 15_000);

  it("rejects 64MiB+1 attachment before send and clears the file input", async () => {
    await mountComposer();
    const input = screen.getByTestId("chat-file-input") as HTMLInputElement;
    fireEvent.change(input, { target: { files: [oversizedFile()] } });
    await waitFor(() => expect(input.value).toBe(""), { timeout: WAIT_TIMEOUT });
    expect(mocks.send).not.toHaveBeenCalled();
    expect(screen.getByTestId("chat-input")).toBeTruthy();
  }, 15_000);

  it("clears the file input after a successful attachment selection", async () => {
    await mountComposer();
    const input = screen.getByTestId("chat-file-input") as HTMLInputElement;
    fireEvent.change(input, { target: { files: [new File(["data"], "note.bin")] } });
    await waitFor(
      () => expect(mocks.send).toHaveBeenCalledWith(PEER, "file", undefined, expect.objectContaining({ name: "note.bin" })),
      { timeout: WAIT_TIMEOUT },
    );
    expect(input.value).toBe("");
  }, 15_000);

  it("does not white-screen when sending fails", async () => {
    mocks.send.mockRejectedValueOnce(new Error("附件失败"));
    await mountComposer();
    const input = screen.getByTestId("chat-file-input");
    fireEvent.change(input, { target: { files: [new File(["x"], "x.bin")] } });
    await waitFor(() => expect(screen.getByTestId("chat-input")).toBeTruthy(), { timeout: WAIT_TIMEOUT });
    expect(screen.getByTestId("chat-file-input")).toBeTruthy();
  }, 15_000);
});

describe("GUI chat event and history boundaries", () => {
  it("shows empty-state guidance and no composer when friend list is empty", async () => {
    mocks.friends.mockResolvedValue([]);
    const { ChatView } = await import("./chat-view");
    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("暂无好友")).toBeTruthy(), { timeout: WAIT_TIMEOUT });
    expect(screen.getByText("暂无会话，选择好友开始聊天")).toBeTruthy();
    expect(screen.queryByTestId("chat-input")).toBeNull();
  }, VIEW_TIMEOUT);

  it("cancels a pending media placeholder; delivered media stays", async () => {
    const pending = mediaMessage("local-1", PEER, "image", chatMedia("photo.png", "image/png", 3));
    const sent = mediaMessage("real-1", PEER, "video", chatMedia("clip.mp4", "video/mp4", 4), { status: "delivered", tsMs: 3 });
    selectConversation([pending, sent]);
    const { ChatView } = await import("./chat-view");
    render(<ChatView />);
    fireEvent.click(screen.getByRole("button", { name: "取消发送" }));
    expect(screen.queryByText("photo.png")).toBeNull();
    expect(screen.queryByRole("button", { name: "取消发送" })).toBeNull();
    expect(screen.getAllByText("clip.mp4").length).toBeGreaterThan(0);
    expect(screen.getByText("已送达")).toBeTruthy();
  }, VIEW_TIMEOUT);

  it("deduplicates chat_message and applies out-of-order status visibly", async () => {
    selectConversation([textMessage("m1", PEER, "在途", { status: "pending" })]);
    const { ChatView } = await import("./chat-view");
    render(<ChatView />);
    emit({ type: "chat_status", peer: PEER, messageId: "m1", status: "delivered" });
    emit({ type: "chat_status", peer: PEER, messageId: "m1", status: "pending" });
    emit({ type: "chat_message", peer: PEER, message: textMessage("m2", PEER, "重复入站", { sender: "them" }) });
    emit({ type: "chat_message", peer: PEER, message: textMessage("m2", PEER, "重复入站", { sender: "them" }) });
    // store 层精确断言（IM-T53）：去重键=消息 id，与 DOM 无关
    const state = useChatStore.getState();
    expect(state.messagesByPeer[PEER].map((m) => m.id)).toEqual(["m1", "m2"]);
    expect(state.lastMessageByPeer[PEER]?.id).toBe("m2");
    // DOM 计数限定在消息流容器：超时 continuation 渲染的游离实例不参与计数
    await waitFor(
      () => expect(within(screen.getByTestId("message-scroll")).getAllByText("重复入站")).toHaveLength(1),
      { timeout: WAIT_TIMEOUT },
    );
    expect(screen.getByText("等待对方上线")).toBeTruthy();
  }, VIEW_TIMEOUT);

  it("renders failed and pending status without an exception", async () => {
    selectConversation([
      textMessage("p1", PEER, "排队", { status: "pending" }),
      textMessage("f1", PEER, "失败", { status: "failed" }),
    ]);
    // 清掉列表摘要，聚焦气泡正文 + 状态角标两处“失败”
    useChatStore.setState({ lastMessageByPeer: {} });
    const { ChatView } = await import("./chat-view");
    render(<ChatView />);
    expect(screen.getByText("排队")).toBeTruthy();
    expect(screen.getAllByText("失败")).toHaveLength(2);
    expect(screen.getAllByTestId("message-status").length).toBe(2);
  }, VIEW_TIMEOUT);

  it("keeps the conversation shell when history rejects or returns an empty page", async () => {
    mocks.history.mockRejectedValueOnce(new Error("非法游标"));
    const { ChatView } = await import("./chat-view");
    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("好友")).toBeTruthy(), { timeout: WAIT_TIMEOUT });
    fireEvent.click(screen.getAllByText("好友")[1]!);
    await waitFor(() => expect(mocks.history).toHaveBeenCalledWith(PEER, null, 50), { timeout: WAIT_TIMEOUT });
    expect(screen.getByRole("textbox")).toBeTruthy();
    expect(screen.getByRole("region", { name: "会话" })).toBeTruthy();
    expect(screen.queryAllByTestId("message-status")).toHaveLength(0);
  }, VIEW_TIMEOUT);
});

describe("GUI media display boundaries", () => {
  it.each([
    ["asset://chat/a.png", "image/png", "IMG"],
    ["blob:http://localhost/a", "image/png", "IMG"],
    ["data:image/png;base64,AAAA", "image/png", "IMG"],
    ["asset://chat/a.mp3", "audio/mpeg", "AUDIO"],
    ["asset://chat/a.mp4", "video/mp4", "VIDEO"],
  ])("renders supported %s media inline", (path, mime, tag) => {
    render(<MediaContent media={{ name: "media", mime, size: 1, path }} />);
    expect(document.querySelector(tag.toLowerCase())).toBeTruthy();
  });

  it("falls back to an information card for a bare path and unknown MIME", () => {
    render(<MediaContent media={{ name: "unknown.bin", mime: "application/x-unknown", size: 0, path: "/tmp/raw.bin" }} />);
    expect(screen.getByText("unknown.bin")).toBeTruthy();
    expect(screen.getByRole("link", { name: "下载" })).toHaveAttribute("href", "/tmp/raw.bin");
  });
});
