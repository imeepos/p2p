import { useEffect, useRef, useState } from "react";
import { MessageSquare } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { ChatMessageJson } from "@/lib/ipc-types";
import { EmptyState } from "@/views/shared/empty-state";

import { MessageBubble } from "./message-bubble";

const LOAD_OLDER_THRESHOLD_PX = 48;
const HIGHLIGHT_MS = 1600;

interface MessageListProps {
  peer: string;
  messages: ChatMessageJson[];
  loadingOlder: boolean;
  hasMore: boolean;
  onLoadOlder: () => void;
  onCancelPending: (messageId: string) => void;
  onReply?: (message: ChatMessageJson) => void;
}

// 消息流容器：向上滚动接近顶部时触发加载更早页（beforeId 游标由 store 管理）；
// 新消息/切换会话自动滚到底部。
// 引用跳转（IM-T46B）：本地历史（当前已加载页）有则滚动居中并短暂高亮；
// 无则由气泡内 QuoteBlock 显示占位文案，不白屏。
export function MessageList({
  peer,
  messages,
  loadingOlder,
  hasMore,
  onLoadOlder,
  onCancelPending,
  onReply,
}: MessageListProps) {
  const { t } = useTranslation();
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const stickBottomRef = useRef(true);
  const highlightTimerRef = useRef<number | null>(null);
  const [highlightId, setHighlightId] = useState<string | null>(null);

  useEffect(() => {
    // switching peer forces stick-to-bottom
    stickBottomRef.current = true;
  }, [peer]);

  useEffect(() => {
    const el = scrollRef.current;
    if (el && stickBottomRef.current) el.scrollTop = el.scrollHeight;
  }, [messages, peer]);

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
    const nearBottom =
      el.scrollHeight - el.scrollTop - el.clientHeight < 64;
    stickBottomRef.current = nearBottom;
    if (el.scrollTop < LOAD_OLDER_THRESHOLD_PX && hasMore && !loadingOlder) {
      onLoadOlder();
    }
  };

  // 被引用消息解析：只认当前已加载的本地历史（messagesByPeer 缓存页）。
  const resolveQuoted = (replyTo: string): ChatMessageJson | undefined =>
    messages.find((m) => m.id === replyTo);

  const openQuote = (message: ChatMessageJson) => {
    const replyTo = message.replyTo;
    if (!replyTo || !resolveQuoted(replyTo)) return;
    const el = scrollRef.current?.querySelector(`[data-message-id="${replyTo}"]`);
    // jsdom 无 scrollIntoView：可选调用，真实浏览器内滚动居中。
    el?.scrollIntoView?.({ block: "center", behavior: "smooth" });
    setHighlightId(replyTo);
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
      data-testid="message-scroll"
      className="flex-1 overflow-y-auto px-4 py-3"
    >
      {loadingOlder ? (
        <p className="py-2 text-center text-xs text-muted-foreground">
          {t("chat.loadingHistory")}
        </p>
      ) : null}
      {!loadingOlder && messages.length === 0 ? (
        <div className="flex h-full items-center justify-center">
          <EmptyState icon={MessageSquare} title={t("chat.noMessages")} />
        </div>
      ) : null}
      <div className="flex flex-col gap-2">
        {messages.map((message) => {
          const replyTo = message.replyTo ?? null;
          const quoted = replyTo ? resolveQuoted(replyTo) ?? null : null;
          return (
            <MessageBubble
              key={message.id}
              message={message}
              onCancelPending={onCancelPending}
              quoted={quoted}
              quotedMissing={replyTo !== null && !quoted}
              highlighted={highlightId === message.id}
              onReply={onReply}
              onQuoteOpen={openQuote}
            />
          );
        })}
      </div>
    </div>
  );
}
