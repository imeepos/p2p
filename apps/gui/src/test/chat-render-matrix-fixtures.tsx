// IM-T47 渲染矩阵共用夹具：store 播种/事件发射/受控 send 延迟/DOM 顺序/挂载工具。
// 消息与好友构造复用 chat-boundaries-fixtures。
import { act, render, screen } from "@testing-library/react";
import { Toaster } from "sonner";

import type { ChatMessageJson, NodeEventHandler } from "@/lib/ipc-types";
import { friendJson, peerId } from "@/test/chat-boundaries-fixtures";
import { useChatStore } from "@/stores/chat-store";
import { ChatView } from "@/views/chat/chat-view";

export const MATRIX_PEER = peerId("matrix-peer");

export function resetChatStore(): void {
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
}

// 直塞 store 选中会话：绕开 loadFriends/selectPeer，聚焦渲染矩阵本身。
export function seedConversation(messages: ChatMessageJson[]): void {
  useChatStore.setState({
    friends: [friendJson(MATRIX_PEER, "矩阵好友")],
    friendsLoaded: true,
    selectedPeer: MATRIX_PEER,
    messagesByPeer: { [MATRIX_PEER]: messages },
    lastMessageByPeer: { [MATRIX_PEER]: messages[messages.length - 1] ?? null },
    historyLoading: {},
    hasMore: { [MATRIX_PEER]: false },
  });
}

export function makeEmitter(handler: { current: NodeEventHandler | null }) {
  return (event: Parameters<NodeEventHandler>[0]): void => {
    act(() => handler.current?.(event));
  };
}

export interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (error: unknown) => void;
}

export function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

// a 位于 b 之前（同一文档序）返回 true。
export function isBefore(a: Element, b: Element): boolean {
  return (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;
}

// 会话列表行：按行内缩略 PeerId 定位，供摘要断言取整行文本。
export function conversationRow(peer: string): HTMLElement {
  const row = screen
    .getAllByRole("button")
    .find((el) => el.textContent?.includes(peer.slice(0, 12)));
  if (!row) throw new Error(`找不到会话行: ${peer}`);
  return row;
}

// 多好友最后消息摘要播种：selectedPeer 置空，聚焦会话列表 summaryOf 渲染。
export function seedSummaries(
  entries: Array<{ peer: string; message: ChatMessageJson }>,
): void {
  useChatStore.setState({
    friends: entries.map((e) => friendJson(e.peer)),
    friendsLoaded: true,
    selectedPeer: null,
    messagesByPeer: {},
    lastMessageByPeer: Object.fromEntries(entries.map((e) => [e.peer, e.message])),
    historyLoading: {},
    historyLoaded: {},
    hasMore: {},
  });
}

export function bubbleArea(): HTMLElement {
  return screen.getByTestId("message-scroll");
}

// 挂载完整聊天页：sonner Toaster 同挂，失败路径可断言错误 toast 实渲染。
export function mountChat(): void {
  render(
    <>
      <Toaster position="bottom-right" />
      <ChatView />
    </>,
  );
}

// jsdom 文件桩：走 Composer 的 file input 真实路径（FileReader 读 base64）。
export function mediaFile(name: string, mime: string, content = "x"): File {
  return new File([content], name, { type: mime });
}
