import { useTranslation } from "react-i18next";

import type { Locale } from "@/i18n";
import { formatTime } from "@/lib/format";
import type { ChatMessageJson } from "@/lib/ipc-types";
import { cn } from "@/lib/utils";

import { MediaContent } from "./media-content";

const STATUS_KEYS = {
  pending: "chat.status.pending",
  sent: "chat.status.sent",
  delivered: "chat.status.delivered",
  failed: "chat.status.failed",
} as const;

interface MessageBubbleProps {
  message: ChatMessageJson;
  onCancelPending?: (messageId: string) => void;
}

// 单条气泡：me 靠右 / them 靠左；文本换行保留；媒体走 MediaContent。
// 状态角标仅 me 消息展示（文字，禁 emoji）；pending 占位可取消（附件未发送）。
export function MessageBubble({ message, onCancelPending }: MessageBubbleProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const isMe = message.sender === "me";
  const pendingPlaceholder =
    isMe && message.status === "pending" && message.kind !== "text";

  return (
    <div className={cn("flex w-full", isMe ? "justify-end" : "justify-start")}>
      <div
        className={cn(
          "max-w-[75%] rounded-lg px-3 py-2 text-sm",
          isMe ? "bg-primary text-primary-foreground" : "bg-muted",
        )}
      >
        {message.kind === "text" && message.text ? (
          <p className="whitespace-pre-wrap break-words">{message.text}</p>
        ) : null}
        {message.media ? <MediaContent media={message.media} /> : null}
        <div className="mt-1 flex items-center gap-2 text-xs opacity-80">
          <time>{formatTime(message.tsMs, locale)}</time>
          {isMe ? (
            <span
              data-testid="message-status"
              className={cn(message.status === "failed" && "font-medium text-destructive")}
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
    </div>
  );
}
