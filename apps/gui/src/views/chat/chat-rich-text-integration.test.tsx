// 富文本渲染集成：入站文本消息经真实挂载面（FriendConversation）走
// BubbleText → TextWithShareLink → RichTextMessage 完整链路，
// 覆盖 GFM/公式/代码高亮/XSS 防御/多条消息增量到达。
import { act, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson, ChatMessageJson, ChatSendReport, NodeEventHandler } from "@/lib/ipc-types";
import { textMessage } from "@/test/chat-boundaries-fixtures";
import {
  MATRIX_PEER,
  bubbleArea,
  mountChat,
  resetChatStore,
  seedConversation,
} from "@/test/chat-render-matrix-fixtures";
import { useChatStore } from "@/stores/chat-store";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    history: vi.fn<(peer: string, beforeId?: string | null, limit?: number) => Promise<ChatMessageJson[]>>(),
    send: vi.fn<() => Promise<ChatSendReport>>(),
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

const emit = (event: Parameters<NodeEventHandler>[0]): void => {
  act(() => {
    for (const handler of mocks.handlers) handler(event);
  });
};
const PEER = MATRIX_PEER;

function inbound(id: string, text: string): void {
  emit({ type: "chat_message", peer: PEER, message: textMessage(id, PEER, text, { sender: "them" }) });
}

beforeEach(async () => {
  vi.clearAllMocks();
  resetChatStore();
  mocks.friends.mockResolvedValue([]);
  mocks.history.mockResolvedValue([]);
  await useChatStore.getState().subscribeEvents();
});

describe("聊天富文本集成·入站渲染", () => {
  it("表格消息按 GFM 渲染表头与单元格", async () => {
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    inbound("rich-table", ["| 名称 | 数量 |", "| - | - |", "| 苹果 | 3 |"].join("\n"));
    const area = bubbleArea();
    expect(area.querySelector("table")).toBeTruthy();
    expect(area.querySelector("thead th")?.textContent).toBe("名称");
    expect(area.querySelector("tbody td")?.textContent).toBe("苹果");
  });

  it("数学公式消息渲染 KaTeX", async () => {
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    inbound("rich-math", "质能方程 $E=mc^2$ 众所周知");
    expect(bubbleArea().querySelector(".katex")).toBeTruthy();
  });

  it("代码块高亮已知语言，未知语言原样可读", async () => {
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    inbound("rich-code-js", ["```js", "const a = 1;", "```"].join("\n"));
    expect(bubbleArea().querySelector("pre code .hljs-keyword")).toBeTruthy();
    inbound("rich-code-x", ["```mysterylang", "opaque content", "```"].join("\n"));
    expect(bubbleArea().querySelectorAll("pre")).toHaveLength(2);
    expect(within(bubbleArea()).getByText("opaque content")).toBeTruthy();
  });

  it("混合富文本消息（标题/列表/引用/加粗）完整渲染", async () => {
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    inbound("rich-mixed", ["## 计划", "- 第一步", "- 第二步", "", "> 引用备注"].join("\n"));
    const area = bubbleArea();
    expect(area.querySelector("h2")?.textContent).toBe("计划");
    expect(area.querySelectorAll("li")).toHaveLength(2);
    expect(area.querySelector("blockquote")).toBeTruthy();
  });
});

describe("聊天富文本集成·XSS 防御", () => {
  it("script 标签消息不产生可执行元素", async () => {
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    inbound("xss-script", "<script>window.__p2p_xss_it = 1;</script>");
    expect(bubbleArea().querySelector("script")).toBeNull();
    expect((window as unknown as Record<string, unknown>).__p2p_xss_it).toBeUndefined();
  });

  it("javascript: 链接消息无可点锚点，链接文字保留", async () => {
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    inbound("xss-link", "看看 [福利](javascript:alert(1)) 快来");
    const area = bubbleArea();
    expect(area.querySelector("a[href^='javascript:']")).toBeNull();
    expect(area.querySelector('[data-testid="chat-rich-text"]')?.querySelector("a")).toBeNull();
    expect(area.textContent).toContain("福利");
  });

  it("img onerror 注入不产生元素", async () => {
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    inbound("xss-img", '<img src=x onerror="window.__p2p_xss_it = 1">');
    expect(bubbleArea().querySelector("img")).toBeNull();
    expect((window as unknown as Record<string, unknown>).__p2p_xss_it).toBeUndefined();
  });
});

describe("聊天富文本集成·增量到达", () => {
  it("多条消息相继到达逐条渲染，互不影响且不崩溃", async () => {
    seedConversation([]);
    mountChat();
    await screen.findByTestId("chat-input");
    inbound("rich-inc-1", "**加粗**第一条");
    inbound("rich-inc-2", "第二条 `行内代码`");
    const richNodes = bubbleArea().querySelectorAll('[data-testid="chat-rich-text"]');
    expect(richNodes).toHaveLength(2);
    expect(bubbleArea().querySelector("strong")?.textContent).toBe("加粗");
    expect(bubbleArea().querySelector("code")?.textContent).toBe("行内代码");
    expect(bubbleArea().querySelector("strong")?.textContent).toBe("加粗");
  });
});
