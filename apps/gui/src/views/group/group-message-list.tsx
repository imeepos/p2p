import { Fragment, useEffect, useLayoutEffect, useRef, useState } from "react";
import { MessagesSquare } from "lucide-react";
import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { toastError } from "@/components/feedback/toast";
import { MessageBubble, type BubbleAvatar } from "@/components/chat/message-bubble";
import { TimeDivider } from "@/components/chat/time-divider";
import { needsTimeDivider } from "@/components/chat/time-divider-rule";
import type { ChatFriendJson, ChatMessageJson, GroupMessageJson } from "@/lib/ipc-types";
import type { Locale } from "@/i18n";
import { errorText } from "@/views/shared/form-flow";
import { EmptyState } from "@/views/shared/empty-state";

import { groupDisplayName, toBubbleMessage } from "./group-names";

const LOAD_OLDER_THRESHOLD_PX = 48;
const HIGHLIGHT_MS = 1600;

interface GroupMessageListProps {
  groupId: string;
  messages: GroupMessageJson[];
  selfPeerId: string | null;
  friends: ChatFriendJson[];
  totalRecipients: number;
  loadingOlder: boolean;
  hasMore: boolean;
  historyError: string | null;
  onLoadOlder: () => void;
  onRetryHistory: () => Promise<unknown>;
  onCancelPending: (messageId: string) => void;
  onReply?: (message: GroupMessageJson) => void;
  /** 群失败文本重发（W1-01）：与 1:1 同入口，透传给气泡 retry 钮 */
  onRetry?: (message: GroupMessageJson) => void;
  /** WX1：me 气泡外侧头像（可选）。 */
  selfAvatar?: BubbleAvatar;
}

interface ItemProps {
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
function GroupBubbleItem({
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
}: ItemProps) {
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

// 群消息流容器：滚动/分页/引用跳转行为与 1:1 MessageList 同款；
// 差异仅在数据源（group-store）与每条的昵称/acks 注入。
export function GroupMessageList({
  groupId,
  messages,
  selfPeerId,
  friends,
  totalRecipients,
  loadingOlder,
  hasMore,
  historyError,
  onLoadOlder,
  onRetryHistory,
  onCancelPending,
  onReply,
  onRetry,
  selfAvatar,
}: GroupMessageListProps) {
  const { t, i18n } = useTranslation();
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const stickBottomRef = useRef(true);
  const lastFirstIdRef = useRef<string | null>(null);
  const lastScrollHeightRef = useRef(0);
  const highlightTimerRef = useRef<number | null>(null);
  const [highlightId, setHighlightId] = useState<string | null>(null);

  useEffect(() => {
    stickBottomRef.current = true;
  }, [groupId]);

  useEffect(() => {
    const el = scrollRef.current;
    if (el && stickBottomRef.current) el.scrollTop = el.scrollHeight;
  }, [messages, groupId]);

  // 向上翻页前插补偿（UX5）：WebKit 无滚动锚定，前插更早历史后视口内容
  // 整体跳位。以「首条消息 id 变化且旧首条仍在列表」识别前插，在布局提交
  // 阶段把 scrollTop 平移高度增量，视口锚定不跳；翻页时用户必然已离开
  // 底部（stickBottom=false），钉底路径不受影响。
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const firstId = messages[0]?.id ?? null;
    const prevFirstId = lastFirstIdRef.current;
    const prepended =
      prevFirstId !== null &&
      firstId !== prevFirstId &&
      messages.some((m) => m.id === prevFirstId);
    if (prepended && !stickBottomRef.current) {
      const delta = el.scrollHeight - lastScrollHeightRef.current;
      if (delta > 0) el.scrollTop += delta;
    }
    lastFirstIdRef.current = firstId;
    lastScrollHeightRef.current = el.scrollHeight;
  }, [messages]);

  useEffect(() => {
    return () => {
      if (highlightTimerRef.current !== null) {
        window.clearTimeout(highlightTimerRef.current);
      }
    };
  }, []);

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    stickBottomRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < 64;
    if (el.scrollTop < LOAD_OLDER_THRESHOLD_PX && hasMore && !loadingOlder) {
      onLoadOlder();
    }
  };

  const resolveQuoted = (message: GroupMessageJson): ChatMessageJson | null => {
    const replyTo = message.replyTo ?? null;
    if (!replyTo) return null;
    const target = messages.find((m) => m.id === replyTo);
    return target ? toBubbleMessage(target, selfPeerId) : null;
  };

  const openQuote = (bubble: ChatMessageJson) => {
    const el = scrollRef.current?.querySelector(
      `[data-message-id="${bubble.id}"]`,
    );
    el?.scrollIntoView?.({ block: "center", behavior: "smooth" });
    setHighlightId(bubble.id);
    if (highlightTimerRef.current !== null) {
      window.clearTimeout(highlightTimerRef.current);
    }
    highlightTimerRef.current = window.setTimeout(() => {
      setHighlightId(null);
      highlightTimerRef.current = null;
    }, HIGHLIGHT_MS);
  };

  return (
    <div
      ref={scrollRef}
      onScroll={onScroll}
      data-testid="group-message-scroll"
      className="scroll-slim min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-4 py-3"
    >
      {historyError ? (
        <div
          data-testid="group-history-error"
          className={messages.length === 0 ? "flex h-full items-center justify-center" : undefined}
        >
          <div className="flex flex-col items-center gap-1.5 text-center">
            <p className="text-destructive text-sm">{t("chat.historyLoadFailed")}</p>
            <p className="max-w-80 text-xs break-all text-muted-foreground">{historyError}</p>
            <AsyncButton
              type="button"
              size="sm"
              variant="outline"
              className="mt-1"
              action={onRetryHistory}
              onError={(error) => {
                console.error("[group] 群历史重试失败", error);
                toastError(t("chat.historyLoadFailed"), {
                  description: errorText(error),
                  context: "group.history_retry",
                });
              }}
            >
              {t("chat.retry")}
            </AsyncButton>
          </div>
        </div>
      ) : null}
      {loadingOlder ? (
        <p className="py-2 text-center text-xs text-muted-foreground">
          {t("chat.loadingHistory")}
        </p>
      ) : null}
      {!loadingOlder && !historyError && messages.length === 0 ? (
        <div className="flex h-full items-center justify-center">
          <EmptyState icon={MessagesSquare} title={t("chat.noMessages")} />
        </div>
      ) : null}
      <div className="flex flex-col gap-y-2.5" data-testid="group-message-column">
        {messages.map((message, index) => (
          <Fragment key={message.id}>
            {needsTimeDivider(index > 0 ? messages[index - 1] : null, message) ? (
              <TimeDivider tsMs={message.tsMs} locale={i18n.language as Locale} />
            ) : null}
            <GroupBubbleItem
              message={message}
              selfPeerId={selfPeerId}
              friends={friends}
              totalRecipients={totalRecipients}
              highlighted={highlightId === message.id}
              quoted={resolveQuoted(message)}
              onCancelPending={onCancelPending}
              onRetry={onRetry}
              onReply={onReply}
              onQuoteOpen={openQuote}
              selfAvatar={selfAvatar}
            />
          </Fragment>
        ))}
      </div>
    </div>
  );
}
