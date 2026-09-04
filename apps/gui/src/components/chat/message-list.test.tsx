import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactElement } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatMessageJson } from "@/lib/ipc-types";
import i18n from "@/i18n";
import { useChatStore } from "@/stores/chat-store";

import { MessageList } from "./message-list";

const { mocks } = vi.hoisted(() => ({
  mocks: { history: vi.fn<() => Promise<ChatMessageJson[]>>() },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: { chatHistory: mocks.history },
}));

const PEER = "3xY9abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWX";

function text(id: string, content: string, tsMs: number): ChatMessageJson {
  return {
    id,
    peer: PEER,
    sender: "me",
    kind: "text",
    tsMs,
    text: content,
    media: null,
    status: "delivered",
  };
}

// 镜像 ChatView 接线：messages 从 store 读入再传 props，重试后消息渲染可断言。
// 选择器只取 record 引用，派生放组件体（`?? []` 内联会让快照不稳定无限重渲）。
function Harness(): ReactElement {
  const messagesByPeer = useChatStore((s) => s.messagesByPeer);
  const hasMoreAll = useChatStore((s) => s.hasMore);
  const messages = messagesByPeer[PEER] ?? [];
  const hasMore = hasMoreAll[PEER] ?? false;
  return (
    <MessageList
      peer={PEER}
      messages={messages}
      loadingOlder={false}
      hasMore={hasMore}
      onLoadOlder={() => {}}
      onCancelPending={() => {}}
    />
  );
}

function seedConversation(messages: ChatMessageJson[]): void {
  useChatStore.setState((s) => ({
    messagesByPeer: { ...s.messagesByPeer, [PEER]: messages },
  }));
}

function seedErrors(history: string | null, older: string | null): void {
  useChatStore.setState((s) => ({
    historyError: { ...s.historyError, [PEER]: history },
    olderError: { ...s.olderError, [PEER]: older },
  }));
}

beforeEach(async () => {
  mocks.history.mockReset();
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

describe("MessageList 历史加载错误态（IM-T50）", () => {
  it("历史失败渲染错误卡与重试入口，不白屏不误报空态", () => {
    seedErrors("历史库锁死", null);
    render(<Harness />);
    expect(screen.getByTestId("chat-history-error")).toBeInTheDocument();
    expect(screen.getByText("历史消息加载失败")).toBeInTheDocument();
    expect(screen.getByText("历史库锁死")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重试" })).toBeInTheDocument();
    expect(screen.queryByText("暂无消息")).not.toBeInTheDocument();
  });

  it("重试成功清除错误并恢复消息渲染", async () => {
    seedErrors("boom", null);
    render(<Harness />);
    mocks.history.mockResolvedValue([text("m1", "恢复的消息", 1000)]);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "重试" }));
    });
    await waitFor(() =>
      expect(useChatStore.getState().historyError[PEER] ?? null).toBeNull(),
    );
    expect(await screen.findByText("恢复的消息")).toBeInTheDocument();
    expect(screen.queryByTestId("chat-history-error")).not.toBeInTheDocument();
  });

  it("loadOlder 失败有顶部信号；重试成功后清除", async () => {
    const m1 = text("m1", "早", 1000);
    seedConversation([m1]);
    useChatStore.setState((s) => ({
      hasMore: { ...s.hasMore, [PEER]: true },
    }));
    seedErrors(null, "cursor lost");
    render(<Harness />);
    expect(screen.getByTestId("chat-older-error")).toBeInTheDocument();
    expect(screen.getByText("更早消息加载失败")).toBeInTheDocument();
    expect(screen.getByText("cursor lost")).toBeInTheDocument();

    mocks.history.mockResolvedValue([]);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "重试" }));
    });
    await waitFor(() =>
      expect(useChatStore.getState().olderError[PEER] ?? null).toBeNull(),
    );
    expect(screen.queryByTestId("chat-older-error")).not.toBeInTheDocument();
  });

  it("en locale 同步：英文错误文案与 Retry 按钮", async () => {
    await i18n.changeLanguage("en-US");
    seedErrors("db locked", null);
    render(<Harness />);
    expect(screen.getByText("Failed to load message history")).toBeInTheDocument();
    expect(screen.getByText("db locked")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });
});
