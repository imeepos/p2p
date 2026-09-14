import { MessageCircle, SearchIcon } from "lucide-react";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { AgentSessionTree } from "@/components/chat/agent-session-tree";
import { ConversationContextMenu } from "@/components/chat/conversation-context-menu";
import { ConversationRow } from "@/components/chat/conversation-row";
import { InvitePlaceholderRow } from "@/components/chat/invite-placeholder-row";
import {
  CONVERSATION_VIRTUAL_THRESHOLD,
  VirtualConversationList,
} from "@/components/chat/virtual-conversation-list";
import { AsyncButton } from "@/components/feedback/async-button";
import { Input } from "@/components/ui/input";
import { filterEntries } from "@/lib/conversation-entry";
import type { ConversationEntry } from "@/lib/conversation-entry";
import type { ContextMenuAnchor } from "@/components/ui/context-menu";
import {
  conversationKey,
  useConversationPrefsStore,
} from "@/stores/conversation-prefs-store";
import type { PendingInviteItem } from "@/views/chat/use-pending-invites";
import { EmptyState } from "@/views/shared/empty-state";

// 会话列表（§2.1/§2.4）：顶部常驻搜索框 + 分段条目。搜索为列表内
// 即时过滤（title/subtitle 子串、ID 前缀、agent host），不做历史全文检索；
// 过滤只影响显示不动排序，清空恢复，无结果显「无匹配会话」空态。
// T5：agent 段嵌接两级树（agent-session-tree 复用 acp/workspace-model），
// 好友/群/a2a 段保持微信风扁平混排，选中/置顶/邀请面板语义不变。
export interface ConversationListProps {
  entries: ConversationEntry[];
  selectedId: string | null;
  /** 任一来源仍在首载时的加载提示 */
  loading: boolean;
  /** 好友簿加载失败原文 + 重试入口（失败路径可观测） */
  error?: string | null;
  onRetry?: () => Promise<void>;
  onSelect: (entry: ConversationEntry) => void;
  /** F17：等待对方同意的邀请占位（与通讯录/消息中心同源），置灰垫底 */
  pendingInvites?: PendingInviteItem[];
}

export function ConversationList({
  entries,
  selectedId,
  loading,
  error,
  onRetry,
  onSelect,
  pendingInvites = [],
}: ConversationListProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [menu, setMenu] = useState<{ entry: ConversationEntry; anchor: ContextMenuAnchor } | null>(
    null,
  );
  const convFlags = useConversationPrefsStore((s) => s.flags);
  const visible = useMemo(() => filterEntries(entries, query), [entries, query]);
  // 占位与真实条目共用搜索词：title/id 子串过滤，保持列表语义一致
  const visiblePending = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return pendingInvites;
    return pendingInvites.filter(
      (p) => p.title.toLowerCase().includes(q) || p.id.toLowerCase().startsWith(q),
    );
  }, [pendingInvites, query]);
  // T5 分段：agent 条目进两级树，其余保持扁平混排（各自相对序不变）
  const agentEntries = useMemo(() => visible.filter((e) => e.kind === "agent"), [visible]);
  const flatEntries = useMemo(() => visible.filter((e) => e.kind !== "agent"), [visible]);
  const mutedOf = (entry: ConversationEntry) =>
    convFlags[conversationKey(entry.kind, entry.id)]?.muted === true;
  const openMenu = (entry: ConversationEntry, anchor: ContextMenuAnchor) =>
    setMenu({ entry, anchor });

  if (loading) {
    return (
      <p className="text-muted-foreground p-4 text-sm">{t("chat.conversations.loading")}</p>
    );
  }
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="px-2.5 pt-2.5 pb-1.5">
        <div className="relative">
          <SearchIcon
            aria-hidden
            className="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2"
          />
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t("chat.conversations.searchPlaceholder")}
            aria-label={t("chat.conversations.searchPlaceholder")}
            data-testid="conversation-search"
            className="border-transparent bg-wx-hover focus-visible:bg-background focus-visible:border-primary/50 h-8 rounded-md pl-8 text-xs"
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
      {entries.length === 0 && pendingInvites.length === 0 ? (
        <EmptyState
          className="min-h-56 flex-1"
          icon={MessageCircle}
          title={t("chat.noFriends")}
          description={t("chat.noFriendsHint")}
        />
      ) : visible.length === 0 && visiblePending.length === 0 ? (
        <EmptyState
          className="min-h-40 flex-1"
          icon={SearchIcon}
          title={t("chat.conversations.noMatch")}
        />
      ) : flatEntries.length + visiblePending.length > CONVERSATION_VIRTUAL_THRESHOLD ? (
        <>
          {/* 虚拟路径：树段行高可变（组头+会话行）不进定高 windowing，改为
              独立限高滚动；agent 端点为手工收藏、量级小，树段恒走普通渲染 */}
          {agentEntries.length > 0 ? (
            <div className="scroll-slim max-h-56 shrink-0 overflow-y-auto border-b border-border/60 pb-1">
              <AgentSessionTree
                entries={agentEntries}
                selectedId={selectedId}
                onSelect={onSelect}
                mutedOf={mutedOf}
                onRowContextMenu={openMenu}
              />
            </div>
          ) : null}
          <VirtualConversationList
            entries={flatEntries}
            pendingInvites={visiblePending}
            selectedId={selectedId}
            onSelect={onSelect}
            mutedOf={mutedOf}
            onRowContextMenu={openMenu}
          />
        </>
      ) : (
        <div
          className="scroll-slim min-h-0 flex-1 overflow-y-auto"
          data-testid="conversation-items"
        >
          {agentEntries.length > 0 ? (
            <div className="pb-1">
              <AgentSessionTree
                entries={agentEntries}
                selectedId={selectedId}
                onSelect={onSelect}
                mutedOf={mutedOf}
                onRowContextMenu={openMenu}
              />
            </div>
          ) : null}
          <ul>
            {flatEntries.map((entry) => (
              <ConversationRow
                key={entry.kind + ":" + entry.id}
                entry={entry}
                active={entry.id === selectedId}
                onSelect={onSelect}
                muted={mutedOf(entry)}
                onContextMenu={(event) => {
                  // 阻止浏览器默认菜单与外层关闭监听（stopPropagation 截断冒泡）
                  event.preventDefault();
                  event.stopPropagation();
                  openMenu(entry, { x: event.clientX, y: event.clientY });
                }}
              />
            ))}
            {visiblePending.map((item) => (
              <InvitePlaceholderRow key={`invite:${item.kind}:${item.id}`} item={item} />
            ))}
          </ul>
        </div>
      )}
      <ConversationContextMenu
        entry={menu?.entry ?? null}
        anchor={menu?.anchor ?? null}
        onClose={() => setMenu(null)}
      />
    </div>
  );
}
