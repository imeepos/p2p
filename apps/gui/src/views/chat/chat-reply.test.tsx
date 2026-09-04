// IM-T46B 回复消息 GUI：回复旅程 / 引用取消 / 缺失占位 / 五类型预览矩阵 /
// 旧消息渲染回归 / 引用跳转高亮。走完整 ChatView（store + 事件注入）。
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ChatFriendJson,
  ChatKind,
  ChatMessageJson,
  ChatSendReport,
  NodeEventHandler,
} from "@/lib/ipc-types";
import {
  chatMedia,
  friendJson,
  mediaMessage,
  peerId,
  sendReport,
  textMessage,
} from "@/test/chat-boundaries-fixtures";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    history: vi.fn<
      (peer: string, beforeId?: string | null, limit?: number) => Promise<ChatMessageJson[]>
    >(),
    send: vi.fn<() => Promise<ChatSendReport>>(),
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

const PEER = peerId("reply-peer");

async function mountWithHistory(history: ChatMessageJson[]): Promise<void> {
  mocks.friends.mockResolvedValue([friendJson(PEER, "引用好友")]);
  mocks.history.mockResolvedValue(history);
  render(<ChatView />);
  fireEvent.click(await screen.findByText("引用好友"));
  await screen.findByTestId("chat-input");
}

// 对端视角注入：mock 事件直接打 chat-store 订阅回调（等同真实 chat_message）。
function emitInbound(message: ChatMessageJson): void {
  act(() => {
    mocks.eventHandler.current?.({ type: "chat_message", peer: PEER, message });
  });
}

function bubbleArea(): HTMLElement {
  return screen.getByTestId("message-scroll");
}

function quotedBlocks(): HTMLElement[] {
  // queryAll：零引用块时返回空数组（旧消息/取消场景要断言"没有"）
  return within(bubbleArea()).queryAllByTestId("chat-quote-block");
}

const LONG_TEXT = "长".repeat(100);

const REPLY_ROWS: Array<{
  kind: ChatKind;
  message: ChatMessageJson;
  expectText: string;
  absentText?: string;
}> = [
  {
    kind: "text",
    message: textMessage("m-text", PEER, LONG_TEXT, { sender: "them", tsMs: 1 }),
    expectText: "长".repeat(80),
    absentText: "长".repeat(81),
  },
  {
    kind: "image",
    message: mediaMessage("m-image", PEER, "image", chatMedia("pic.png", "image/png"), { sender: "them", tsMs: 2 }),
    expectText: "图片",
  },
  {
    kind: "audio",
    message: mediaMessage("m-audio", PEER, "audio", chatMedia("voice.mp3", "audio/mpeg"), { sender: "them", tsMs: 3 }),
    expectText: "音频",
  },
  {
    kind: "video",
    message: mediaMessage("m-video", PEER, "video", chatMedia("clip.mp4", "video/mp4"), { sender: "them", tsMs: 4 }),
    expectText: "视频",
  },
  {
    kind: "file",
    message: mediaMessage("m-file", PEER, "file", chatMedia("年度报告.pdf", "application/pdf"), { sender: "them", tsMs: 5 }),
    expectText: "年度报告.pdf",
  },
];

beforeEach(() => {
  vi.resetAllMocks();
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

describe("回复消息 GUI", () => {
  it("回复旅程：收到消息→点回复→输入区预览→发送携带 replyTo→气泡含引用块→入站消息携带 replyTo 渲染引用", async () => {
    await mountWithHistory([textMessage("in-1", PEER, "这条要回复", { sender: "them", tsMs: 1000 })]);
    fireEvent.click(screen.getByTestId("message-reply-in-1"));
    const preview = screen.getByTestId("chat-reply-preview");
    expect(preview.textContent).toContain("这条要回复");

    mocks.send.mockResolvedValue(
      sendReport({ ...textMessage("out-1", PEER, "好的收到", { tsMs: 2000 }), replyTo: "in-1" }),
    );
    fireEvent.change(screen.getByTestId("chat-input"), { target: { value: "好的收到" } });
    fireEvent.click(screen.getByTestId("chat-send"));

    await waitFor(() =>
      expect(mocks.send).toHaveBeenCalledWith(PEER, "text", "好的收到", undefined, "in-1"),
    );
    // 发送成功后引用预览消失；我的气泡带引用块（摘要=被引用文本）
    await waitFor(() => expect(screen.queryByTestId("chat-reply-preview")).toBeNull());
    await waitFor(() => expect(quotedBlocks()).toHaveLength(1));
    expect(quotedBlocks()[0].textContent).toContain("这条要回复");

    // 对方视角：入站消息携带 replyTo（mock 事件注入），气泡渲染引用块且不缺占位
    emitInbound({ ...textMessage("in-2", PEER, "引用你的回复", { sender: "them", tsMs: 3000 }), replyTo: "out-1" });
    await waitFor(() => expect(quotedBlocks()).toHaveLength(2));
    expect(screen.queryByTestId("chat-quote-missing")).toBeNull();
  });

  it("引用取消：取消预览后发送不带 replyTo", async () => {
    await mountWithHistory([textMessage("in-c", PEER, "被取消的引用", { sender: "them", tsMs: 1000 })]);
    fireEvent.click(screen.getByTestId("message-reply-in-c"));
    expect(screen.getByTestId("chat-reply-preview")).toBeTruthy();
    fireEvent.click(screen.getByTestId("chat-reply-cancel"));
    expect(screen.queryByTestId("chat-reply-preview")).toBeNull();

    mocks.send.mockResolvedValue(sendReport(textMessage("out-c", PEER, "普通发送", { tsMs: 2000 })));
    fireEvent.change(screen.getByTestId("chat-input"), { target: { value: "普通发送" } });
    fireEvent.click(screen.getByTestId("chat-send"));
    await waitFor(() => expect(mocks.send).toHaveBeenCalled());
    // 恰好 3 参 = 无 replyTo；上屏气泡也无引用块
    expect(mocks.send.mock.calls[0]).toEqual([PEER, "text", "普通发送"]);
    await waitFor(() => expect(bubbleArea().textContent).toContain("普通发送"));
    expect(quotedBlocks()).toHaveLength(0);
    expect(screen.queryByTestId("chat-quote-missing")).toBeNull();
  });

  it("被引用消息不在本地：占位文案渲染不白屏", async () => {
    await mountWithHistory([
      { ...textMessage("ghost-1", PEER, "引用未知消息", { sender: "them", tsMs: 1000 }), replyTo: "no-such-id" },
    ]);
    expect(screen.getByTestId("chat-quote-missing").textContent).toContain("引用消息不在本地");
    // 不白屏：输入区与会话区壳仍在
    expect(screen.getByTestId("chat-input")).toBeTruthy();
    expect(screen.getByRole("region", { name: "会话" })).toBeTruthy();
  });

  it.each(REPLY_ROWS)("$kind：引用预览按类型显示 i18n 摘要", async (row) => {
    await mountWithHistory([row.message]);
    fireEvent.click(screen.getByTestId(`message-reply-${row.message.id}`));
    const preview = screen.getByTestId("chat-reply-preview");
    expect(preview.textContent).toContain(row.expectText);
    if (row.absentText) {
      expect(preview.textContent).not.toContain(row.absentText); // 文本截断生效
    }
  });

  it("旧消息兼容：无 replyTo 渲染零回归", async () => {
    await mountWithHistory([
      textMessage("old-t", PEER, "旧文本消息", { sender: "me", tsMs: 1, status: "delivered" }),
      mediaMessage("old-i", PEER, "image", chatMedia("old.png", "image/png", 8, "/data/old.png"), { sender: "them", tsMs: 2 }),
    ]);
    expect(screen.queryByTestId("chat-quote-block")).toBeNull();
    expect(screen.queryByTestId("chat-quote-missing")).toBeNull();
    expect(screen.queryByTestId("chat-reply-preview")).toBeNull();
    expect(screen.getByText("旧文本消息")).toBeTruthy();
    expect(within(bubbleArea()).getByText("old.png")).toBeTruthy();
  });

  it("引用跳转：点击引用块定位并短暂高亮被引用气泡，随后高亮清除", async () => {
    await mountWithHistory([
      textMessage("jt-1", PEER, "跳转目标", { sender: "them", tsMs: 1000 }),
      { ...textMessage("jt-2", PEER, "带引用回复", { sender: "them", tsMs: 2000 }), replyTo: "jt-1" },
    ]);
    const target = bubbleArea().querySelector('[data-message-id="jt-1"]');
    const replier = bubbleArea().querySelector('[data-message-id="jt-2"]');
    expect(target).toBeTruthy();
    expect(replier).toBeTruthy();
    fireEvent.click(within(replier as HTMLElement).getByTestId("chat-quote-block"));
    await waitFor(() =>
      expect(target?.getAttribute("data-highlighted")).toBe("true"),
    );
    // 短暂高亮：约 1.6s 后自动清除（真实定时器）
    await waitFor(
      () => expect(target?.getAttribute("data-highlighted")).toBeNull(),
      { timeout: 3000 },
    );
  });
});
