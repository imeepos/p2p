import { act, render, screen } from "@testing-library/react";
import type { ReactElement } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import { mergeMessages } from "@/lib/chat-local";
import type { ChatMessageJson, NodeEventJson } from "@/lib/ipc-types";
import { createMockChatBackend } from "@/lib/mock-chat";
import { forceMockMessageStatus, injectMockIncoming } from "@/lib/mock-chat-inject";
import i18n from "@/i18n";
import { useChatStore } from "@/stores/chat-store";

import { MessageList } from "./message-list";

// 44 字符合法 base58（chatFriendAdd 校验 43-45 字符）。
const PEER = "3xY9abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQ";
const SELF = "2xY9abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQ";

const events: NodeEventJson[] = [];
const backend = createMockChatBackend({
  emit: (event) => events.push(event),
  selfPeerId: () => SELF,
  isRunning: () => false,
  isConnected: () => false,
  addKnownPeer: () => {},
});

// 镜像 ChatView 接线：store record 引用选择（避免 `?? []` 内联快照不稳）。
function Harness(): ReactElement {
  const messagesByPeer = useChatStore((s) => s.messagesByPeer);
  const messages = messagesByPeer[PEER] ?? [];
  return (
    <MessageList
      peer={PEER}
      messages={messages}
      loadingOlder={false}
      hasMore={false}
      onLoadOlder={() => {}}
      onCancelPending={() => {}}
    />
  );
}

function seedStore(messages: ChatMessageJson[]): void {
  useChatStore.setState((s) => ({
    messagesByPeer: { ...s.messagesByPeer, [PEER]: messages },
    lastMessageByPeer: {
      ...s.lastMessageByPeer,
      [PEER]: messages[messages.length - 1] ?? null,
    },
  }));
}

beforeEach(async () => {
  events.length = 0;
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
    historyError: {},
    olderError: {},
  });
  await i18n.changeLanguage("zh-CN");
});

describe("mock 场景注入渲染（IM-T50）", () => {
  it("注入 them 文本/图片气泡渲染靠左，chat_message 事件与历史一致", async () => {
    injectMockIncoming(PEER, { kind: "text", text: "对方文本消息" });
    injectMockIncoming(PEER, {
      kind: "image",
      media: { name: "photo.png", mime: "image/png", dataBase64: "aGk=" },
    });

    expect(events.filter((e) => e.type === "chat_message")).toHaveLength(2);
    const history = await backend.chatHistory(PEER, null, 50);
    expect(history).toHaveLength(2);
    expect(history.every((m) => m.sender === "them")).toBe(true);
    seedStore(history);
    render(<Harness />);

    expect(screen.getByText("对方文本消息")).toBeInTheDocument();
    // mock 占位路径非内联资源：图片退化为信息卡展示文件名
    expect(screen.getByText("photo.png")).toBeInTheDocument();
    const themNode = screen.getByText("对方文本消息").closest("[data-message-id]");
    expect(themNode?.className).toContain("justify-start");
  });

  it("me 消息经 forceMockMessageStatus 推进 pending→failed 渲染失败角标", async () => {
    await backend.chatFriendAdd(PEER, "对端", []);
    const sent = await backend.chatSend(PEER, "text", "我的待发消息");
    expect(sent.message.status).toBe("pending");
    seedStore([sent.message]);
    render(<Harness />);
    expect(screen.getByTestId("message-status")).toHaveTextContent("等待对方上线");

    const failed = forceMockMessageStatus(PEER, sent.message.id, "failed");
    expect(failed.status).toBe("failed");
    act(() => {
      useChatStore.setState((s) => ({
        messagesByPeer: {
          ...s.messagesByPeer,
          [PEER]: mergeMessages(s.messagesByPeer[PEER] ?? [], [failed]),
        },
      }));
    });
    expect(screen.getByTestId("message-status")).toHaveTextContent("失败");
  });
});
