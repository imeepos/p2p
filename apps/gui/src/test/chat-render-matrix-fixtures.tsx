// IM-T47 渲染矩阵共用夹具：store 播种/事件发射/受控 send 延迟/DOM 顺序/挂载工具。
// 消息与好友构造复用 chat-boundaries-fixtures。
import { act, render, screen } from "@testing-library/react";
import { Toaster } from "sonner";

import type { ChatMessageJson, NodeEventHandler } from "@/lib/ipc-types";
import { friendJson, peerId } from "@/test/chat-boundaries-fixtures";
import { useChatStore } from "@/stores/chat-store";
import { FriendConversation } from "@/views/chat/friend-conversation";
import { ConversationList } from "@/components/chat/conversation-list";
import { friendEntry, sortEntries, type PreviewLabels } from "@/lib/conversation-entry";

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

// 统一会话列表行（P1 条目模型）：按 testid 定位，供摘要断言取整行文本。
export function conversationRow(peer: string): HTMLElement {
  const row = screen.queryByTestId("conversation-row-friend-" + peer);
  if (!row) throw new Error("找不到会话行: " + peer);
  return row;
}

// 多好友最后消息摘要播种：selectedPeer 置空，聚焦会话列表预览渲染。
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

// 挂载选中会话（P1 右栏）：sonner Toaster 同挂，失败路径可断言错误 toast 实渲染。
// MATRIX_PEER 的会话须先 seedConversation（或 selectPeer）播种。
export function mountChat(): void {
  render(
    <>
      <Toaster position="bottom-right" />
      <FriendConversation peer={MATRIX_PEER} />
    </>,
  );
}

// 挂载统一会话列表（P1）：条目按生产同款 friendEntry 构建器从 store 聚合。
// 独立于 mountChat：列表断言与右栏断言互不依赖。
export function mountConversationListFromStore(): void {
  const s = useChatStore.getState();
  const labels: PreviewLabels = {
    image: "[图片]",
    audio: "[语音]",
    video: "[视频]",
    file: "[文件]",
    self: "我",
  };
  const entries = sortEntries(
    s.friends.map((friend, index) =>
      friendEntry({
        friend,
        last: s.lastMessageByPeer[friend.peerId] ?? null,
        unread: s.unreadByPeer[friend.peerId] ?? 0,
        joinSeq: index,
        labels,
      }),
    ),
  );
  render(
    <ConversationList
      entries={entries}
      selectedId={null}
      loading={false}
      onSelect={() => {}}
    />,
  );
}

// jsdom 文件桩：走 Composer 的 file input 真实路径（FileReader 读 base64）。
export function mediaFile(name: string, mime: string, content = "x"): File {
  return new File([content], name, { type: mime });
}
