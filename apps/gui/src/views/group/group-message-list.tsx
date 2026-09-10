import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { MessagesSquare } from "lucide-react";
import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { toastError } from "@/components/feedback/toast";
import { LoadingHistoryHint } from "@/components/chat/history-notices";
import { MESSAGE_VIRTUAL_THRESHOLD } from "@/components/chat/message-list";
import type { BubbleAvatar } from "@/components/chat/message-bubble";
import type { ChatFriendJson, ChatMessageJson, GroupMessageJson } from "@/lib/ipc-types";
import { errorText } from "@/views/shared/form-flow";
import { EmptyState } from "@/views/shared/empty-state";

import { GroupMessageRow } from "./group-bubble-item";
import { toBubbleMessage } from "./group-names";
import { VirtualGroupMessageList } from "./virtual-group-message-list";
import type { MessageFlowHandle } from "@/components/chat/virtual-message-flow";

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

// 群消息流容器：滚动/分页/引用跳转行为与 1:1 MessageList 同款；长列表
// （>阈值）切 react-virtuoso 虚拟路径，DOM 节点数与消息总量解耦。
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
  const { t } = useTranslation();
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const flowRef = useRef<MessageFlowHandle | null>(null);
  const stickBottomRef = useRef(true);
  const lastFirstIdRef = useRef<string | null>(null);
  const lastScrollHeightRef = useRef(0);
  const highlightTimerRef = useRef<number | null>(null);
  const [highlightId, setHighlightId] = useState<string | null>(null);

  const virtual = messages.length > MESSAGE_VIRTUAL_THRESHOLD;

  useEffect(() => {
    stickBottomRef.current = true;
  }, [groupId]);

  useEffect(() => {
    const el = scrollRef.current;
    if (el && stickBottomRef.current && !virtual) el.scrollTop = el.scrollHeight;
  }, [messages, groupId, virtual]);

  // 向上翻页前插补偿（UX5）：WebKit 无滚动锚定，前插更早历史后视口内容
  // 整体跳位。以「首条消息 id 变化且旧首条仍在列表」识别前插，在布局提交
  // 阶段把 scrollTop 平移高度增量，视口锚定不跳；翻页时用户必然已离开
  // 底部（stickBottom=false），钉底路径不受影响。虚拟路径由 firstItemIndex
  // 锚定等价替代。
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el || virtual) return;
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
  }, [messages, virtual]);

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
    if (virtual) {
      const index = messages.findIndex((m) => m.id === bubble.id);
      if (index >= 0) flowRef.current?.scrollToIndex(index);
    } else {
      const el = scrollRef.current?.querySelector(`[data-message-id="${bubble.id}"]`);
      el?.scrollIntoView?.({ block: "center", behavior: "smooth" });
    }
    if (highlightTimerRef.current !== null) {
      window.clearTimeout(highlightTimerRef.current);
    }
    setHighlightId(bubble.id);
    highlightTimerRef.current = window.setTimeout(() => {
      setHighlightId(null);
      highlightTimerRef.current = null;
    }, HIGHLIGHT_MS);
  };

  if (virtual) {
    return (
      <VirtualGroupMessageList
        flowRef={flowRef}
        groupId={groupId}
        messages={messages}
        selfPeerId={selfPeerId}
        friends={friends}
        totalRecipients={totalRecipients}
        loadingOlder={loadingOlder}
        hasMore={hasMore}
        onLoadOlder={onLoadOlder}
        onCancelPending={onCancelPending}
        onReply={onReply}
        onRetry={onRetry}
        selfAvatar={selfAvatar}
        highlightId={highlightId}
        openQuote={openQuote}
      />
    );
  }

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
      {loadingOlder ? <LoadingHistoryHint /> : null}
      {!loadingOlder && !historyError && messages.length === 0 ? (
        <div className="flex h-full items-center justify-center">
          <EmptyState icon={MessagesSquare} title={t("chat.noMessages")} />
        </div>
      ) : null}
      <div className="flex flex-col gap-y-2.5" data-testid="group-message-column">
        {messages.map((message, index) => (
          <GroupMessageRow
            key={message.id}
            message={message}
            prev={index > 0 ? messages[index - 1] : null}
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
            quoted={resolveQuoted(message)}
            highlighted={highlightId === message.id}
          />
        ))}
      </div>
    </div>
  );
}
