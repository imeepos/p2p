import { FolderInputIcon, Trash2Icon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { PeerStatusDot } from "@/components/chat/peer-status";
import { Button } from "@/components/ui/button";
import type { Locale } from "@/i18n";
import { formatTime } from "@/lib/format";
import type { ChatFriendJson, ChatMessageJson } from "@/lib/ipc-types";
import { cn } from "@/lib/utils";
import { usePeerOnline } from "@/stores/node-store";

interface FriendRowProps {
  friend: ChatFriendJson;
  last: ChatMessageJson | null;
  summary: string | null;
  isActive: boolean;
  onSelect: (peerId: string) => void;
  onMoveFriend?: (peerId: string) => void;
  onRemoveFriend?: (peerId: string) => void;
}

// 单行独立组件：在线点按 peerId 订阅，状态翻转只重渲该行不扩散整表。
// 分组操作（IM-T43）：悬停出现「移动到分组」（移出分组 = 对话框内清空提交）。
export function FriendRow({
  friend,
  last,
  summary,
  isActive,
  onSelect,
  onMoveFriend,
  onRemoveFriend,
}: FriendRowProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const online = usePeerOnline(friend.peerId);
  return (
    <li className="group relative">
      <button
        type="button"
        onClick={() => onSelect(friend.peerId)}
        className={cn(
          "w-full px-3 py-2 text-left text-sm hover:bg-accent",
          isActive && "bg-accent",
        )}
      >
        <div className="flex items-center justify-between gap-2">
          <span className="flex min-w-0 items-center gap-1.5">
            <PeerStatusDot
              online={online}
              testId={`chat-peer-status-${friend.peerId}`}
            />
            <span className="truncate font-medium">
              {friend.nickname || friend.peerId.slice(0, 8)}
            </span>
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
      <div className="absolute top-1/2 right-2 flex -translate-y-1/2 gap-1 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
        {onMoveFriend ? (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label={t("chat.group.moveAction")}
            title={t("chat.group.moveAction")}
            data-testid={`chat-move-friend-${friend.peerId}`}
            onClick={() => onMoveFriend(friend.peerId)}
          >
            <FolderInputIcon aria-hidden />
          </Button>
        ) : null}
        {onRemoveFriend ? (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label={t("chat.removeFriend.action")}
            title={t("chat.removeFriend.action")}
            data-testid={`chat-remove-friend-${friend.peerId}`}
            onClick={() => onRemoveFriend(friend.peerId)}
          >
            <Trash2Icon aria-hidden />
          </Button>
        ) : null}
      </div>
    </li>
  );
}
