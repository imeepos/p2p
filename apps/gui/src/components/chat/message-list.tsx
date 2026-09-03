import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import type { ChatMessageJson } from "@/lib/ipc-types";

import { MessageBubble } from "./message-bubble";

const LOAD_OLDER_THRESHOLD_PX = 48;

interface MessageListProps {
  peer: string;
  messages: ChatMessageJson[];
  loadingOlder: boolean;
  hasMore: boolean;
  onLoadOlder: () => void;
  onCancelPending: (messageId: string) => void;
}

// 消息流容器：向上滚动接近顶部时触发加载更早页（beforeId 游标由 store 管理）；
// 新消息/切换会话自动滚到底部。
export function MessageList({
  peer,
  messages,
  loadingOlder,
  hasMore,
  onLoadOlder,
  onCancelPending,
}: MessageListProps) {
  const { t } = useTranslation();
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const stickBottomRef = useRef(true);

  useEffect(() => {
    // switching peer forces stick-to-bottom
    stickBottomRef.current = true;
  }, [peer]);

  useEffect(() => {
    const el = scrollRef.current;
    if (el && stickBottomRef.current) el.scrollTop = el.scrollHeight;
  }, [messages, peer]);

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    const nearBottom =
      el.scrollHeight - el.scrollTop - el.clientHeight < 64;
    stickBottomRef.current = nearBottom;
    if (el.scrollTop < LOAD_OLDER_THRESHOLD_PX && hasMore && !loadingOlder) {
      onLoadOlder();
    }
  };

  return (
    <div
      ref={scrollRef}
      onScroll={onScroll}
      data-testid="message-scroll"
      className="flex-1 overflow-y-auto px-4 py-3"
    >
      {loadingOlder ? (
        <p className="py-2 text-center text-xs text-muted-foreground">
          {t("chat.loadingHistory")}
        </p>
      ) : null}
      <div className="flex flex-col gap-2">
        {messages.map((message) => (
          <MessageBubble
            key={message.id}
            message={message}
            onCancelPending={onCancelPending}
          />
        ))}
      </div>
    </div>
  );
}
