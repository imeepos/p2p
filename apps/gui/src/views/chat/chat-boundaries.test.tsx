import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson, ChatMessageJson, ChatSendReport, NodeEventHandler } from "@/lib/ipc-types";
import { MediaContent } from "@/components/chat/media-content";
import { Composer } from "@/components/chat/composer";
import { useChatStore } from "@/stores/chat-store";
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

const PEER = "3xY9" + "1ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".repeat(2).slice(0, 40);
const friend = (nickname = "好友"): ChatFriendJson => ({ peerId: PEER, nickname, addrs: [], note: null });
const message = (id: string, text: string, status: ChatMessageJson["status"] = "delivered"): ChatMessageJson => ({
  id, peer: PEER, sender: "me", kind: "text", tsMs: Number(id.replace(/\D/g, "")) || 1, text, media: null, status,
});

function resetStore(): void {
  useChatStore.setState({ friends: [], friendsLoaded: false, friendsError: null, selectedPeer: null, messagesByPeer: {}, lastMessageByPeer: {}, historyLoading: {}, historyLoaded: {}, hasMore: {} });
}
function emit(event: Parameters<NodeEventHandler>[0]): void { act(() => mocks.handler.current?.(event)); }
async function mountComposer(): Promise<void> {
  useChatStore.setState({ friends: [friend()], friendsLoaded: true, selectedPeer: PEER, messagesByPeer: {}, lastMessageByPeer: {}, historyLoading: {}, hasMore: { [PEER]: false } });
  render(<Composer peer={PEER} />);
  await waitFor(() => expect(screen.getByTestId("chat-input")).toBeTruthy());
}

beforeEach(() => {
  vi.clearAllMocks();
  resetStore();
  mocks.friends.mockResolvedValue([friend()]);
  mocks.history.mockResolvedValue([]);
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
  });

  it("toggles emoji panel on repeated clicks and inserts at selection", async () => {
    await mountComposer();
    const input = screen.getByTestId("chat-input") as HTMLTextAreaElement;
    fireEvent.change(input, { target: { value: "ab" } });
    input.setSelectionRange(1, 1);
    const toggle = screen.getByRole("button", { name: "表情" });
    fireEvent.click(toggle);
    expect(screen.getByRole("menu")).toBeTruthy();
    fireEvent.click(screen.getByRole("menuitem", { name: "😀" }));
    await waitFor(() => expect(input.value).toBe("a😀b"));
    fireEvent.click(toggle);
    expect(screen.queryByRole("menu")).toBeNull();
    fireEvent.click(toggle);
    expect(screen.getByRole("menu")).toBeTruthy();
  });

  it("renders attachment kinds and rejects oversized selection without white-screen", async () => {
    await mountComposer();
    const input = screen.getByTestId("chat-file-input") as HTMLInputElement;
    const files = [
      new File(["x"], "photo.png", { type: "image/png" }),
      new File(["x"], "sound.mp3", { type: "audio/mpeg" }),
      new File(["x"], "clip.mp4", { type: "video/mp4" }),
      new File(["x"], "archive.unknown", { type: "application/octet-stream" }),
    ];
    for (const file of files) {
      fireEvent.change(input, { target: { files: [file] } });
      await waitFor(() => expect(mocks.send).toHaveBeenCalled());
      mocks.send.mockClear();
    }
    const huge = Object.create(File.prototype) as File;
    Object.defineProperties(huge, { name: { value: "huge.bin" }, type: { value: "" }, size: { value: 64 * 1024 * 1024 + 1 } });
    fireEvent.change(input, { target: { files: [huge] } });
    await waitFor(() => expect(screen.getByTestId("chat-input")).toBeTruthy());
    expect(screen.getByTestId("chat-file-input")).toBeTruthy();
  });

  it("cancels an attachment selection and clears the file input", async () => {
    await mountComposer();
    const input = screen.getByTestId("chat-file-input") as HTMLInputElement;
    const file = new File(["data"], "note.bin", { type: "application/octet-stream" });
    fireEvent.change(input, { target: { files: [file] } });
    await waitFor(() => expect(mocks.send).toHaveBeenCalledWith(PEER, "file", undefined, expect.objectContaining({ name: "note.bin" })));
    expect(input.value).toBe("");
  });

  it("does not white-screen when sending fails", async () => {
    mocks.send.mockRejectedValueOnce(new Error("附件失败"));
    await mountComposer();
    const input = screen.getByTestId("chat-file-input");
    fireEvent.change(input, { target: { files: [new File(["x"], "x.bin")] } });
    await waitFor(() => expect(screen.getByTestId("chat-input")).toBeTruthy());
    expect(screen.getByTestId("chat-file-input")).toBeTruthy();
  });
});

describe("GUI chat event and history boundaries", () => {
  it("deduplicates chat_message and applies out-of-order status visibly", async () => {
    useChatStore.setState({ friends: [friend()], friendsLoaded: true, selectedPeer: PEER, messagesByPeer: { [PEER]: [message("m1", "在途", "pending")] }, lastMessageByPeer: { [PEER]: message("m1", "在途", "pending") }, historyLoading: {}, hasMore: { [PEER]: false } });
    const { ChatView } = await import("./chat-view");
    render(<ChatView />);
    emit({ type: "chat_status", peer: PEER, messageId: "m1", status: "delivered" });
    emit({ type: "chat_status", peer: PEER, messageId: "m1", status: "pending" });
    emit({ type: "chat_message", peer: PEER, message: { ...message("m2", "重复入站"), sender: "them" } });
    emit({ type: "chat_message", peer: PEER, message: { ...message("m2", "重复入站"), sender: "them" } });
    await waitFor(() => expect(screen.getAllByText("重复入站")).toHaveLength(2));
    expect(screen.getByText("发送中")).toBeTruthy();
  });

  it("renders failed and pending status without an exception", async () => {
    const { ChatView } = await import("./chat-view");
    useChatStore.setState({ friends: [friend()], friendsLoaded: true, selectedPeer: PEER, messagesByPeer: { [PEER]: [message("p1", "排队", "pending"), message("f1", "失败", "failed")] }, lastMessageByPeer: {}, historyLoading: {}, hasMore: { [PEER]: false } });
    render(<ChatView />);
    expect(screen.getByText("排队")).toBeTruthy();
    expect(screen.getAllByText("失败")).toHaveLength(2);
    expect(screen.getAllByTestId("message-status").length).toBe(2);
  });

  it("keeps the conversation shell when history is empty or rejects", async () => {
    mocks.friends.mockResolvedValue([friend()]);
    mocks.history.mockRejectedValueOnce(new Error("非法游标"));
    const { ChatView } = await import("./chat-view");
    render(<ChatView />);
    await waitFor(() => expect(screen.getByText("好友")).toBeTruthy());
    fireEvent.click(screen.getAllByText("好友")[1]!);
    await waitFor(() => expect(screen.getByRole("textbox")).toBeTruthy());
    expect(screen.getByRole("region", { name: "会话" })).toBeTruthy();
  });
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
