import { MessageCircle, SearchIcon } from "lucide-react";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { ConversationRow } from "@/components/chat/conversation-row";
import { AsyncButton } from "@/components/feedback/async-button";
import { Input } from "@/components/ui/input";
import { filterEntries } from "@/lib/conversation-entry";
import type { ConversationEntry } from "@/lib/conversation-entry";
import { EmptyState } from "@/views/shared/empty-state";

// 会话列表（§2.1/§2.4）：顶部常驻搜索框 + 统一条目混排。搜索为列表内
// 即时过滤（title/subtitle 子串、ID 前缀、agent host），不做历史全文检索；
// 过滤只影响显示不动排序，清空恢复，无结果显「无匹配会话」空态。
export interface ConversationListProps {
  entries: ConversationEntry[];
  selectedId: string | null;
  /** 任一来源仍在首载时的加载提示 */
  loading: boolean;
  /** 好友簿加载失败原文 + 重试入口（失败路径可观测） */
  error?: string | null;
  onRetry?: () => Promise<void>;
  onSelect: (entry: ConversationEntry) => void;
}

export function ConversationList({
  entries,
  selectedId,
  loading,
  error,
  onRetry,
  onSelect,
}: ConversationListProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const visible = useMemo(() => filterEntries(entries, query), [entries, query]);

  if (loading) {
    return (
      <p className="text-muted-foreground p-4 text-sm">{t("chat.conversations.loading")}</p>
    );
  }
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="border-b p-2">
        <div className="relative">
          <SearchIcon
            aria-hidden
            className="text-muted-foreground pointer-events-none absolute top-1/2 left-2 size-4 -translate-y-1/2"
          />
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t("chat.conversations.searchPlaceholder")}
            aria-label={t("chat.conversations.searchPlaceholder")}
            data-testid="conversation-search"
            className="pl-8"
          />
        </div>
      </div>
      {error ? (
        <div className="flex flex-col items-start gap-2 p-3">
          <p className="text-destructive text-sm">{error}</p>
          {onRetry ? (
            <AsyncButton
              type="button"
              size="sm"
              variant="outline"
              action={onRetry}
              onError={(retryError) => {
                console.error("[chat] 会话列表重试失败", retryError);
              }}
            >
              {t("common.actions.refresh")}
            </AsyncButton>
          ) : null}
        </div>
      ) : null}
      {entries.length === 0 ? (
        <EmptyState
          className="min-h-56 flex-1"
          icon={MessageCircle}
          title={t("chat.noFriends")}
          description={t("chat.noFriendsHint")}
        />
      ) : visible.length === 0 ? (
        <EmptyState
          className="min-h-40 flex-1"
          icon={SearchIcon}
          title={t("chat.conversations.noMatch")}
        />
      ) : (
        <ul
          className="scroll-slim min-h-0 flex-1 divide-y overflow-y-auto"
          data-testid="conversation-items"
        >
          {visible.map((entry) => (
            <ConversationRow
              key={entry.kind + ":" + entry.id}
              entry={entry}
              active={entry.id === selectedId}
              onSelect={onSelect}
            />
          ))}
        </ul>
      )}
    </div>
  );
}
