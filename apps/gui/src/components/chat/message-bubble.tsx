import { Reply } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import type { Locale } from "@/i18n";
import { formatTime } from "@/lib/format";
import type { ChatMessageJson } from "@/lib/ipc-types";
import { cn } from "@/lib/utils";

import { MediaContent } from "./media-content";
import { QuoteBlock } from "./quote-block";
import { replySummaryOf } from "./reply-summary";

const STATUS_KEYS = {
  pending: "chat.status.pending",
  sent: "chat.status.sent",
  delivered: "chat.status.delivered",
  failed: "chat.status.failed",
} as const;

interface MessageBubbleProps {
  message: ChatMessageJson;
  onCancelPending?: (messageId: string) => void;
  /** 被引用消息（本地历史可解析时）；与 quotedMissing 二选一。 */
  quoted?: ChatMessageJson | null;
  quotedMissing?: boolean;
  /** 引用跳转的短暂高亮态（由 MessageList 定时清除）。 */
  highlighted?: boolean;
  onReply?: (message: ChatMessageJson) => void;
  onQuoteOpen?: (message: ChatMessageJson) => void;
}

// 回复入口：悬停/键盘聚焦可见，位于气泡外侧；不干扰气泡本体点击。
function ReplyButton({
  message,
  isMe,
  onReply,
}: {
  message: ChatMessageJson;
  isMe: boolean;
  onReply: (message: ChatMessageJson) => void;
}) {
  const { t } = useTranslation();
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon"
      className={cn(
        "absolute top-1/2 size-6 -translate-y-1/2 opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100",
        isMe ? "left-full ml-1" : "right-full mr-1",
      )}
      aria-label={t("chat.reply.action")}
      title={t("chat.reply.action")}
      data-testid={`message-reply-${message.id}`}
      onClick={() => onReply(message)}
    >
      <Reply aria-hidden className="size-3.5" />
    </Button>
  );
}

// 单条气泡：me 靠右 / them 靠左；文本换行保留；媒体走 MediaContent。
// 状态角标仅 me 消息展示（文字，禁 emoji）；pending 占位可取消（附件未发送）。
// 引用块（IM-T46B）：replyTo 指向本地可解析消息时渲染摘要，缺失时占位文案。
export function MessageBubble({
  message,
  onCancelPending,
  quoted,
  quotedMissing = false,
  highlighted = false,
  onReply,
  onQuoteOpen,
}: MessageBubbleProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const isMe = message.sender === "me";
  const pendingPlaceholder =
    isMe && message.status === "pending" && message.kind !== "text";
  const tone = isMe ? "me" : "them";

  return (
    <div
      className={cn("group relative flex w-full", isMe ? "justify-end" : "justify-start")}
      data-message-id={message.id}
      data-highlighted={highlighted ? "true" : undefined}
    >
      <div
        className={cn(
          "max-w-[75%] rounded-lg px-3 py-2 text-sm",
          isMe ? "bg-primary text-primary-foreground" : "bg-muted",
          highlighted && "ring-2 ring-primary",
        )}
      >
        {quoted ? (
          <QuoteBlock
            kind={quoted.kind}
            summary={replySummaryOf(quoted)}
            missing={false}
            tone={tone}
            onOpen={() => onQuoteOpen?.(message)}
          />
        ) : null}
        {quotedMissing ? (
          <QuoteBlock
            summary={null}
            missing={true}
            tone={tone}
            onOpen={() => onQuoteOpen?.(message)}
          />
        ) : null}
        {message.kind === "text" && message.text ? (
          <p className="whitespace-pre-wrap break-words">{message.text}</p>
        ) : null}
        {message.media ? <MediaContent media={message.media} /> : null}
        <div className="mt-1 flex items-center gap-2 text-xs opacity-80">
          <time>{formatTime(message.tsMs, locale)}</time>
          {isMe ? (
            <span
              data-testid="message-status"
              className={cn(
                message.status === "failed" &&
                  "font-medium text-red-300 dark:text-red-700",
              )}
            >
              {t(STATUS_KEYS[message.status])}
            </span>
          ) : null}
          {pendingPlaceholder && onCancelPending ? (
            <button
              type="button"
              onClick={() => onCancelPending(message.id)}
              className="underline underline-offset-2"
            >
              {t("chat.cancelSend")}
            </button>
          ) : null}
        </div>
      </div>
      {onReply ? <ReplyButton message={message} isMe={isMe} onReply={onReply} /> : null}
    </div>
  );
}
