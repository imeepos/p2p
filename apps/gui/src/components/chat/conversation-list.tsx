import { MessageCircle, Trash2Icon, UserPlusIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import type { Locale } from "@/i18n";
import { formatTime } from "@/lib/format";
import type { ChatFriendJson, ChatMessageJson } from "@/lib/ipc-types";
import { cn } from "@/lib/utils";
import { EmptyState } from "@/views/shared/empty-state";

interface ConversationListProps {
  friends: ChatFriendJson[];
  lastMessages: Record<string, ChatMessageJson | null>;
  selectedPeer: string | null;
  loading: boolean;
  error: string | null;
  onSelect: (peerId: string) => void;
  onAddFriend?: () => void;
  onRemoveFriend?: (peerId: string) => void;
}

function summaryOf(message: ChatMessageJson | null | undefined): string | null {
  if (!message) return null;
  if (message.kind === "text") return message.text ?? null;
  return message.media?.name ?? null;
}

// 会话列表：昵称（空回退 PeerId 缩略）+ 缩略 PeerId + 最后消息摘要 + 时间；
// 按最后消息时间倒序（无消息排末尾）。
export function ConversationList({
  friends,
  lastMessages,
  selectedPeer,
  loading,
  error,
  onSelect,
  onAddFriend,
  onRemoveFriend,
}: ConversationListProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;

  if (loading) {
    return <p className="p-4 text-sm text-muted-foreground">{t("common.state.unknown")}</p>;
  }
  if (error) {
    return <p className="p-4 text-sm text-destructive">{error}</p>;
  }
  if (friends.length === 0) {
    return (
      <EmptyState
        className="min-h-56"
        icon={MessageCircle}
        title={t("chat.noFriends")}
        description={t("chat.noFriendsHint")}
        action={
          onAddFriend ? (
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={onAddFriend}
              data-testid="chat-add-friend-empty"
            >
              <UserPlusIcon aria-hidden />
              {t("chat.addFriend.action")}
            </Button>
          ) : undefined
        }
      />
    );
  }

  const sorted = [...friends].sort((a, b) => {
    const ta = lastMessages[a.peerId]?.tsMs ?? 0;
    const tb = lastMessages[b.peerId]?.tsMs ?? 0;
    return tb - ta;
  });

  return (
    <ul className="divide-y">
      {sorted.map((friend) => {
        const last = lastMessages[friend.peerId] ?? null;
        const summary = summaryOf(last);
        const isActive = selectedPeer === friend.peerId;
        return (
          <li key={friend.peerId} className="group relative">
            <button
              type="button"
              onClick={() => onSelect(friend.peerId)}
              className={cn(
                "w-full px-3 py-2 text-left text-sm hover:bg-accent",
                isActive && "bg-accent",
              )}
            >
              <div className="flex items-baseline justify-between gap-2">
                <span className="truncate font-medium">
                  {friend.nickname || friend.peerId.slice(0, 8)}
                </span>
                {last ? (
                  <time className="shrink-0 text-xs text-muted-foreground">
                    {formatTime(last.tsMs, locale)}
                  </time>
                ) : null}
              </div>
              <div className="text-xs text-muted-foreground">
                {friend.peerId.slice(0, 12)}
              </div>
              <div className="truncate text-xs text-muted-foreground">
                {summary ?? ""}
              </div>
            </button>
            {onRemoveFriend ? (
              <Button
                type="button"
                variant="ghost"
                size="icon"
                className="absolute top-1/2 right-2 -translate-y-1/2 opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
                aria-label={t("chat.removeFriend.action")}
                title={t("chat.removeFriend.action")}
                data-testid={`chat-remove-friend-${friend.peerId}`}
                onClick={() => onRemoveFriend(friend.peerId)}
              >
                <Trash2Icon aria-hidden />
              </Button>
            ) : null}
          </li>
        );
      })}
    </ul>
  );
}
