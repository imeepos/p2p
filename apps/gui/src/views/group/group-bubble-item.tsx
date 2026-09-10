import { useTranslation } from "react-i18next";

import type { Locale } from "@/i18n";
import { MessageBubble, type BubbleAvatar } from "@/components/chat/message-bubble";
import { TimeDivider } from "@/components/chat/time-divider";
import { needsTimeDivider } from "@/components/chat/time-divider-rule";
import type { ChatFriendJson, ChatMessageJson, GroupMessageJson } from "@/lib/ipc-types";

import { groupDisplayName, toBubbleMessage } from "./group-names";

export interface GroupBubbleItemProps {
  message: GroupMessageJson;
  selfPeerId: string | null;
  friends: ChatFriendJson[];
  totalRecipients: number;
  highlighted: boolean;
  quoted: ChatMessageJson | null;
  onCancelPending: (messageId: string) => void;
  onReply?: (message: GroupMessageJson) => void;
  onRetry?: (message: GroupMessageJson) => void;
  onQuoteOpen: (bubble: ChatMessageJson) => void;
  selfAvatar?: BubbleAvatar;
}

// 单条群气泡：昵称标签（them）与送达计数（me，acks 推导「已送达 k/n」）
// 经扩展 props 注入 1:1 MessageBubble，渲染路径零分叉。
// 普通路径与虚拟化路径共用，两条渲染路径行结构完全一致。
export function GroupBubbleItem({
  message,
  selfPeerId,
  friends,
  totalRecipients,
  highlighted,
  quoted,
  onCancelPending,
  onReply,
  onRetry,
  onQuoteOpen,
  selfAvatar,
}: GroupBubbleItemProps) {
  const { t } = useTranslation();
  const view = toBubbleMessage(message, selfPeerId);
  const isMe = view.sender === "me";
  // 群发送状态（W1-01）：pending 显发送中、failed 走 1:1 气泡原生失败+重试
  // 渲染（不传 override），仅已落账的 sent/delivered 显送达计数。
  const statusOverride =
    isMe && message.status !== "pending" && message.status !== "failed"
      ? t("group.delivery", { acked: message.acks.length, total: totalRecipients })
      : undefined;
  return (
    <MessageBubble
      message={view}
      senderLabel={isMe ? undefined : groupDisplayName(message.senderId, friends)}
      avatar={
        isMe
          ? selfAvatar
          : { label: groupDisplayName(message.senderId, friends), seed: message.senderId }
      }
      statusOverride={statusOverride}
      highlighted={highlighted}
      quoted={quoted}
      quotedMissing={view.replyTo !== null && !quoted}
      onCancelPending={onCancelPending}
      onRetry={onRetry ? () => onRetry(message) : undefined}
      onReply={onReply ? () => onReply(message) : undefined}
      onQuoteOpen={onQuoteOpen}
    />
  );
}

// 群消息行：时间分割线 + 群气泡。普通路径与虚拟化路径共用。
export function GroupMessageRow({
  message,
  prev,
  itemProps,
  highlighted,
  quoted,
}: {
  message: GroupMessageJson;
  prev: GroupMessageJson | null;
  itemProps: Omit<GroupBubbleItemProps, "message" | "highlighted" | "quoted">;
  highlighted: boolean;
  quoted: ChatMessageJson | null;
}) {
  const { i18n } = useTranslation();
  const divider = needsTimeDivider(prev, message) ? (
    <TimeDivider tsMs={message.tsMs} locale={i18n.language as Locale} />
  ) : null;
  return (
    <>
      {divider}
      <GroupBubbleItem message={message} highlighted={highlighted} quoted={quoted} {...itemProps} />
    </>
  );
}
