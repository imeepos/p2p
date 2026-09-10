import type { Locale } from "@/i18n";
import type { ChatMessageJson } from "@/lib/ipc-types";
import { matchInviteForMessage } from "@/lib/group-invite-match";
import { useChatStore } from "@/stores/chat-store";

import type { GroupInviteTarget } from "./group-invite-dialog";
import { GroupInviteCard } from "./group-invite-card";
import { MessageBubble, type BubbleAvatar } from "./message-bubble";
import { TimeDivider } from "./time-divider";
import { needsTimeDivider } from "./time-divider-rule";

export interface MessageRowContext {
  language: Locale;
  groupInvites: ReturnType<typeof useChatStore.getState>["groupInvites"];
  onCancelPending: (messageId: string) => void;
  onReply?: (message: ChatMessageJson) => void;
  onRetry?: (message: ChatMessageJson) => void;
  resolveQuoted: (replyTo: string) => ChatMessageJson | undefined;
  onQuoteOpen: (message: ChatMessageJson) => void;
  onInviteTarget: (target: GroupInviteTarget) => void;
  selfAvatar?: BubbleAvatar;
  peerAvatar?: BubbleAvatar;
}

// 单条消息行：时间分割线（与前一条跨天/跨段时）+ 气泡 / 入群邀请卡。
// 普通路径与虚拟化路径共用，两条渲染路径行结构完全一致。
export function MessageRow({
  message,
  prev,
  ctx,
  highlighted,
}: {
  message: ChatMessageJson;
  prev: ChatMessageJson | null;
  ctx: MessageRowContext;
  highlighted: boolean;
}) {
  const divider = needsTimeDivider(prev, message) ? (
    <TimeDivider tsMs={message.tsMs} locale={ctx.language} />
  ) : null;
  if (message.kind === "groupInvite") {
    return (
      <>
        {divider}
        <GroupInviteCard
          message={message}
          invite={matchInviteForMessage(message, ctx.groupInvites)}
          onOpenConfirm={(invite) => ctx.onInviteTarget({ message, invite })}
        />
      </>
    );
  }
  const replyTo = message.replyTo ?? null;
  const quoted = replyTo ? ctx.resolveQuoted(replyTo) ?? null : null;
  return (
    <>
      {divider}
      <MessageBubble
        message={message}
        onCancelPending={ctx.onCancelPending}
        quoted={quoted}
        quotedMissing={replyTo !== null && !quoted}
        highlighted={highlighted}
        onReply={ctx.onReply}
        onQuoteOpen={ctx.onQuoteOpen}
        onRetry={ctx.onRetry}
        avatar={message.sender === "me" ? ctx.selfAvatar : ctx.peerAvatar}
      />
    </>
  );
}
