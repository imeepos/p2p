import { Reply } from "lucide-react";
import { useTranslation } from "react-i18next";

import { AvatarBox } from "@/components/chat/avatar-box";
import { CopyButton } from "@/components/feedback/copy-button";
import { Button } from "@/components/ui/button";
import type { ChatMessageJson } from "@/lib/ipc-types";
import { cn } from "@/lib/utils";

import { findLlmShareLinkInText } from "@/lib/llm-share-link-model";

import { MediaContent } from "./media-content";
import { TextWithLlmShareLink } from "./llm-share-message-card";
import { QuoteBlock } from "./quote-block";
import { replySummaryOf } from "./reply-summary";
import { TextWithShareLink } from "./share-message-card";

const STATUS_KEYS = {
  pending: "chat.status.pending",
  sent: "chat.status.sent",
  delivered: "chat.status.delivered",
  failed: "chat.status.failed",
} as const;

export interface BubbleAvatar {
  /** 回退首字符与稳定取色种子 */
  label: string;
  seed: string;
  src?: string | null;
}

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
  /** 失败重发（IM-T51）：仅 me+failed+text 提供入口；媒体走重新选择提示。 */
  onRetry?: (message: ChatMessageJson) => void;
  /** 群聊扩展（G3）：them 气泡上方的发送者展示名；1:1 不传不渲染。 */
  senderLabel?: string;
  /** 群聊扩展（G3）：me 气泡状态行覆盖（已送达 k/n）；不传走原状态文案。 */
  statusOverride?: string;
  /** WX1：气泡外侧头像；不传保持无头像布局（兼容既有渲染矩阵）。 */
  avatar?: BubbleAvatar;
}

// 回复入口：悬停/键盘聚焦可见，锚定气泡列、落在行内空白侧；
// 绝不越出行边界（越界会撑出横向滚动条），不干扰气泡本体点击。
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
        isMe ? "right-full mr-1" : "left-full ml-1",
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

// 正文双 scheme 分发（W4）：llm-share 链接独立 finder 优先（dsh-llm-share://），
// ACP 固定前缀其次；无链接走纯文本。两域渲染互不干扰。
function BubbleText({ text }: { text: string }) {
  if (findLlmShareLinkInText(text)) return <TextWithLlmShareLink text={text} />;
  return <TextWithShareLink text={text} />;
}

// 条目复制值：仅文本消息可复制；trim 首尾空白（空格/回车/换行/制表符等），
// 全空白视为无可复制内容。气泡显示文本不受影响。
function copyableTextOf(message: ChatMessageJson): string | null {
  if (message.kind !== "text" || !message.text) return null;
  const trimmed = message.text.trim();
  return trimmed.length > 0 ? trimmed : null;
}

// WX1 微信风格气泡：me 靠右绿泡 / them 靠左白泡，各带指向头像的小尾巴；
// 头像在气泡外侧（可选，兼容不传场景）。发送状态仅 me 消息展示（泡内小字）。
// 引用块（IM-T46B）：replyTo 指向本地可解析消息时渲染摘要，缺失时占位文案。
export function MessageBubble({
  message,
  onCancelPending,
  quoted,
  quotedMissing = false,
  highlighted = false,
  onReply,
  onQuoteOpen,
  onRetry,
  senderLabel,
  statusOverride,
  avatar,
}: MessageBubbleProps) {
  const { t } = useTranslation();
  const isMe = message.sender === "me";
  const pendingPlaceholder =
    isMe && message.status === "pending" && message.kind !== "text";
  const tone = isMe ? "me" : "them";
  const copyValue = copyableTextOf(message);

  return (
    <div
      className={cn(
        "group relative flex w-full items-start gap-2.5",
        isMe ? "flex-row-reverse" : "flex-row",
      )}
      data-message-id={message.id}
      data-highlighted={highlighted ? "true" : undefined}
    >
      {avatar ? (
        <AvatarBox
          label={avatar.label}
          seed={avatar.seed}
          src={avatar.src}
          size="md"
          className="mt-0.5"
        />
      ) : null}
      <div className={cn("relative flex min-w-0 max-w-[65%] flex-col", isMe && "items-end")}>
        {!isMe && senderLabel ? (
          <div
            className="text-muted-foreground mb-0.5 px-0.5 text-xs"
            data-testid="group-sender-label"
          >
            {senderLabel}
          </div>
        ) : null}
        <div
          className={cn(
            "wx-bubble rounded-lg px-3 py-2 text-sm shadow-sm",
            isMe ? "wx-bubble-me bg-wx-bubble-me" : "wx-bubble-them bg-wx-bubble-them",
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
            <BubbleText text={message.text} />
          ) : null}
          {message.media ? <MediaContent media={message.media} /> : null}
          {isMe ? (
            <div className="mt-1 flex items-center justify-end gap-2 text-[11px] opacity-70">
              <span
                data-testid="message-status"
                className={cn(
                  message.status === "failed" &&
                    "font-medium text-red-600 opacity-100 dark:text-red-300",
                )}
              >
                {statusOverride ?? t(STATUS_KEYS[message.status])}
              </span>
              {pendingPlaceholder && onCancelPending ? (
                <button
                  type="button"
                  onClick={() => onCancelPending(message.id)}
                  className="underline underline-offset-2"
                >
                  {t("chat.cancelSend")}
                </button>
              ) : null}
              {isMe && message.status === "failed" && message.kind === "text" && onRetry ? (
                <button
                  type="button"
                  data-testid={`message-retry-${message.id}`}
                  className="underline underline-offset-2"
                  onClick={() => onRetry(message)}
                >
                  {t("chat.retry")}
                </button>
              ) : null}
              {isMe && message.status === "failed" && message.kind !== "text" ? (
                <span data-testid={`media-retry-hint-${message.id}`}>
                  {t("chat.mediaRetryHint")}
                </span>
              ) : null}
            </div>
          ) : null}
        </div>
        {onReply ? <ReplyButton message={message} isMe={isMe} onReply={onReply} /> : null}
        {copyValue ? (
          <CopyButton
            value={copyValue}
            className={cn(
              "absolute top-1/2 size-6 -translate-y-1/2 opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100",
              // 与回复钮同侧错位（回复钮 mr-1/ml-1 占 28px），落气泡空白侧不越行。
              isMe ? "right-full mr-9" : "left-full ml-9",
            )}
            data-testid={`message-copy-${message.id}`}
          />
        ) : null}
      </div>
    </div>
  );
}