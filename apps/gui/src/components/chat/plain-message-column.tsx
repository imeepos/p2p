import { Fragment } from "react";
import { MessageSquare } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { ChatMessageJson } from "@/lib/ipc-types";
import { EmptyState } from "@/views/shared/empty-state";

import { HistoryErrorNotice, OlderErrorBanner } from "./history-notices";
import type { MessageRowContext } from "./message-row";
import { MessageRow } from "./message-row";

interface PlainMessageColumnProps {
  messages: ChatMessageJson[];
  scrollRef: React.RefObject<HTMLDivElement>;
  onScroll: () => void;
  loadingOlder: boolean;
  historyError: string | null;
  olderError: string | null;
  onRetryHistory: () => Promise<unknown>;
  onRetryOlder: () => Promise<unknown>;
  rowContext: MessageRowContext;
  highlightId: string | null;
}

// 短列表普通渲染路径：全量 DOM，滚动/前插补偿行为与历史实现一致。
// 长列表（>阈值）由 VirtualMessageFlow 接管，行为语义保持一致。
export function PlainMessageColumn({
  messages,
  scrollRef,
  onScroll,
  loadingOlder,
  historyError,
  olderError,
  onRetryHistory,
  onRetryOlder,
  rowContext,
  highlightId,
}: PlainMessageColumnProps) {
  const { t } = useTranslation();
  return (
    <div
      ref={scrollRef}
      onScroll={onScroll}
      data-testid="message-scroll"
      className="scroll-slim min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-4 py-3"
    >
      {olderError ? (
        <OlderErrorBanner detail={olderError} onRetry={onRetryOlder} />
      ) : null}
      {loadingOlder ? (
        <p className="py-2 text-center text-xs text-muted-foreground">
          {t("chat.loadingHistory")}
        </p>
      ) : null}
      {historyError ? (
        <div
          className={
            messages.length === 0 ? "flex h-full items-center justify-center" : undefined
          }
        >
          <HistoryErrorNotice detail={historyError} onRetry={onRetryHistory} />
        </div>
      ) : null}
      {!loadingOlder && !historyError && messages.length === 0 ? (
        <div className="flex h-full items-center justify-center">
          <EmptyState icon={MessageSquare} title={t("chat.noMessages")} />
        </div>
      ) : null}
      <div className="flex flex-col gap-y-2.5" data-testid="message-column">
        {messages.map((message, index) => (
          <Fragment key={message.id}>
            <MessageRow
              message={message}
              prev={index > 0 ? messages[index - 1] : null}
              ctx={rowContext}
              highlighted={highlightId === message.id}
            />
          </Fragment>
        ))}
      </div>
    </div>
  );
}
