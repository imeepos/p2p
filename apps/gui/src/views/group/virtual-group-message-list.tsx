import type { Ref } from "react";

import type { BubbleAvatar } from "@/components/chat/message-bubble";
import {
  VirtualMessageFlow,
  type MessageFlowHandle,
} from "@/components/chat/virtual-message-flow";
import type { ChatFriendJson, ChatMessageJson, GroupMessageJson } from "@/lib/ipc-types";

import { GroupMessageRow } from "./group-bubble-item";
import { toBubbleMessage } from "./group-names";

export interface VirtualGroupMessageListProps {
  flowRef: Ref<MessageFlowHandle>;
  groupId: string;
  messages: GroupMessageJson[];
  selfPeerId: string | null;
  friends: ChatFriendJson[];
  totalRecipients: number;
  loadingOlder: boolean;
  hasMore: boolean;
  onLoadOlder: () => void;
  onCancelPending: (messageId: string) => void;
  onReply?: (message: GroupMessageJson) => void;
  onRetry?: (message: GroupMessageJson) => void;
  selfAvatar?: BubbleAvatar;
  highlightId: string | null;
  openQuote: (bubble: ChatMessageJson) => void;
}

// 群消息流虚拟路径（react-virtuoso）：滚动/分页/引用跳转行为与普通路径
// 同款，差异仅在每条的昵称/acks 注入。DOM 节点数与消息总量解耦。
export function VirtualGroupMessageList({
  flowRef,
  messages,
  selfPeerId,
  friends,
  totalRecipients,
  loadingOlder,
  hasMore,
  onLoadOlder,
  onCancelPending,
  onReply,
  onRetry,
  selfAvatar,
  highlightId,
  openQuote,
}: VirtualGroupMessageListProps) {
  const resolveQuoted = (message: GroupMessageJson): ChatMessageJson | null => {
    const replyTo = message.replyTo ?? null;
    if (!replyTo) return null;
    const target = messages.find((m) => m.id === replyTo);
    return target ? toBubbleMessage(target, selfPeerId) : null;
  };
  return (
    <VirtualMessageFlow<GroupMessageJson>
      ref={flowRef}
      items={messages}
      itemId={(m) => m.id}
      scrollerTestId="group-message-scroll"
      canLoadOlder={hasMore}
      loadingOlder={loadingOlder}
      onTopReached={onLoadOlder}
      highlightId={highlightId}
      renderItem={(message, prev, highlighted) => (
        <GroupMessageRow
          message={message}
          prev={prev}
          highlighted={highlighted}
          quoted={resolveQuoted(message)}
          itemProps={{
            selfPeerId,
            friends,
            totalRecipients,
            onCancelPending,
            onRetry,
            onReply,
            onQuoteOpen: openQuote,
            selfAvatar,
          }}
        />
      )}
    />
  );
}
