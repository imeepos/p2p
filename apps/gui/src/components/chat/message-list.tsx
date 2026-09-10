import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import type { ChatMessageJson } from "@/lib/ipc-types";
import { useChatStore } from "@/stores/chat-store";

import { GroupInviteDialog, type GroupInviteTarget } from "./group-invite-dialog";
import { MessageRow, type MessageRowContext } from "./message-row";
import { PlainMessageColumn } from "./plain-message-column";
import {
  usePrependScrollCompensation,
  useStickToBottom,
} from "./plain-scroll-hooks";
import {
  VirtualMessageFlow,
  type MessageFlowHandle,
} from "./virtual-message-flow";

// 虚拟化阈值（长列表优化）：低于该值走普通渲染（全量 DOM、行为与历史实现
// 完全一致），超过则切 react-virtuoso 虚拟流，DOM 节点数与消息总量解耦。
export const MESSAGE_VIRTUAL_THRESHOLD = 200;

const HIGHLIGHT_MS = 1600;

interface MessageListProps {
  peer: string;
  messages: ChatMessageJson[];
  loadingOlder: boolean;
  hasMore: boolean;
  onLoadOlder: () => void;
  onCancelPending: (messageId: string) => void;
  onReply?: (message: ChatMessageJson) => void;
  /** 失败文本重发入口（IM-T51）：透传给 me+failed+text 气泡。 */
  onRetry?: (message: ChatMessageJson) => void;
  /** WX1：气泡外侧头像（可选，缺省保持无头像布局）。 */
  selfAvatar?: MessageRowContext["selfAvatar"];
  peerAvatar?: MessageRowContext["peerAvatar"];
}

// 消息流容器：普通/虚拟双路径，行为语义一致（钉底跟随、加载更早、引用
// 跳转高亮）。引用跳转（IM-T46B）：本地历史（当前已加载页）有则滚动定位
// 并短暂高亮；无则由气泡内 QuoteBlock 显示占位文案，不白屏。
export function MessageList({
  peer,
  messages,
  loadingOlder,
  hasMore,
  onLoadOlder,
  onCancelPending,
  onReply,
  onRetry,
  selfAvatar,
  peerAvatar,
}: MessageListProps) {
  const { i18n } = useTranslation();
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const flowRef = useRef<MessageFlowHandle | null>(null);
  const highlightTimerRef = useRef<number | null>(null);
  const [highlightId, setHighlightId] = useState<string | null>(null);
  // 加载失败信号（IM-T50）：按本组件 peer 从 store 读取；重试直接复用
  // selectPeer/loadOlder（失败态下二者必然重新拉取）。
  const historyError = useChatStore((s) => s.historyError[peer] ?? null);
  const olderError = useChatStore((s) => s.olderError[peer] ?? null);
  const selectPeerAction = useChatStore((s) => s.selectPeer);
  const loadOlderAction = useChatStore((s) => s.loadOlder);
  // IMC3：入群邀请卡片态与确认弹框（1:1 流专属；群流在 GroupMessageList 降级）。
  const groupInvites = useChatStore((s) => s.groupInvites);
  const [inviteTarget, setInviteTarget] = useState<GroupInviteTarget | null>(null);

  const virtual = messages.length > MESSAGE_VIRTUAL_THRESHOLD;
  // 虚拟路径下普通路径的滚动钩子保持惰性（空数组不触发任何滚动副作用）
  const stickBottomRef = useStickToBottom(scrollRef, virtual ? [] : messages, peer);
  usePrependScrollCompensation(scrollRef, virtual ? [] : messages, stickBottomRef);

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
    if (el.scrollTop < 48 && hasMore && !loadingOlder) {
      onLoadOlder();
    }
  };

  const resolveQuoted = (replyTo: string): ChatMessageJson | undefined =>
    messages.find((m) => m.id === replyTo);

  const openQuote = (message: ChatMessageJson) => {
    const replyTo = message.replyTo;
    if (!replyTo || !resolveQuoted(replyTo)) return;
    if (virtual) {
      const index = messages.findIndex((m) => m.id === replyTo);
      if (index >= 0) flowRef.current?.scrollToIndex(index);
    } else {
      const el = scrollRef.current?.querySelector(`[data-message-id="${replyTo}"]`);
      // jsdom 无 scrollIntoView：可选调用，真实浏览器内滚动居中。
      el?.scrollIntoView?.({ block: "center", behavior: "smooth" });
    }
    if (highlightTimerRef.current !== null) {
      window.clearTimeout(highlightTimerRef.current);
    }
    setHighlightId(replyTo);
    highlightTimerRef.current = window.setTimeout(() => {
      setHighlightId(null);
      highlightTimerRef.current = null;
    }, HIGHLIGHT_MS);
  };

  const rowContext: MessageRowContext = {
    language: i18n.language as MessageRowContext["language"],
    groupInvites,
    onCancelPending,
    onReply,
    onRetry,
    resolveQuoted,
    onQuoteOpen: openQuote,
    onInviteTarget: setInviteTarget,
    selfAvatar,
    peerAvatar,
  };

  return (
    <div className="min-h-0 flex-1">
      {virtual ? (
        <VirtualMessageFlow
          ref={flowRef}
          items={messages}
          itemId={(m) => m.id}
          scrollerTestId="message-scroll"
          canLoadOlder={hasMore}
          loadingOlder={loadingOlder}
          onTopReached={onLoadOlder}
          olderError={olderError}
          onRetryOlder={() => loadOlderAction(peer)}
          highlightId={highlightId}
          renderItem={(message, prev, highlighted) => (
            <MessageRow
              message={message}
              prev={prev}
              ctx={rowContext}
              highlighted={highlighted}
            />
          )}
        />
      ) : (
        <PlainMessageColumn
          messages={messages}
          scrollRef={scrollRef}
          onScroll={onScroll}
          loadingOlder={loadingOlder}
          historyError={historyError}
          olderError={olderError}
          onRetryHistory={() => selectPeerAction(peer)}
          onRetryOlder={() => loadOlderAction(peer)}
          rowContext={rowContext}
          highlightId={highlightId}
        />
      )}
      {inviteTarget ? (
        <GroupInviteDialog
          target={inviteTarget}
          onOpenChange={(open) => {
            if (!open) setInviteTarget(null);
          }}
        />
      ) : null}
    </div>
  );
}
