import type { ChatMessageJson, NodeEventJson } from "@/lib/ipc-types";
import { mergeMessages } from "@/lib/chat-local";

// chat_message 事件归并（§2.3 未读切面）：入站消息去重追加并刷新摘要；
// 本端未选中该会话且非自己发送时 unread+1。返回 null = 事件不触及缓存。
export interface ChatEventStateSlice {
  selectedPeer: string | null;
  messagesByPeer: Record<string, ChatMessageJson[]>;
  lastMessageByPeer: Record<string, ChatMessageJson | null>;
  unreadByPeer: Record<string, number>;
}

export function reduceChatMessage(
  s: ChatEventStateSlice,
  event: Extract<NodeEventJson, { type: "chat_message" }>,
): Partial<ChatEventStateSlice> | null {
  const peer = event.message.peer;
  const list = s.messagesByPeer[peer] ?? [];
  if (list.some((m) => m.id === event.message.id)) return null;
  const unreadByPeer = { ...s.unreadByPeer };
  // §2.3：自己发出的消息不计入；选中会话不计入（清零路径归 selectPeer）
  if (event.message.sender !== "me" && s.selectedPeer !== peer) {
    unreadByPeer[peer] = (unreadByPeer[peer] ?? 0) + 1;
  }
  return {
    messagesByPeer: {
      ...s.messagesByPeer,
      [peer]: mergeMessages(list, [event.message]),
    },
    lastMessageByPeer: { ...s.lastMessageByPeer, [peer]: event.message },
    unreadByPeer,
  };
}
