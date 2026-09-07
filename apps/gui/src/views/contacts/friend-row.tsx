import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";
import { MessageSquareIcon, MoveIcon, Trash2Icon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { CopyButton } from "@/components/monitor/copy-button";
import { initialOf } from "@/lib/conversation-entry";
import type { ChatFriendJson } from "@/lib/ipc-types";
import { usePeerOnline } from "@/stores/node-store";
import { PeerStatusDot } from "@/components/chat/peer-status";

interface FriendRowProps {
  friend: ChatFriendJson;
  onMove: (friend: ChatFriendJson) => void;
  onRemove: (friend: ChatFriendJson) => void;
}

// 好友行（§3.1）：头像首字 + 昵称 + 备注 + 分组名 + 在线状态点（复用
// peer-status）；行内操作：发消息（/chat?peer=）、移动分组、删除。
export function FriendRow({ friend, onMove, onRemove }: FriendRowProps) {
  const { t } = useTranslation();
  const online = usePeerOnline(friend.peerId);
  const name = friend.nickname || friend.peerId.slice(0, 8);
  return (
    <div
      className="hover:bg-accent/50 flex items-center gap-3 rounded-md px-2 py-2"
      data-testid={"contact-friend-" + friend.peerId}
    >
      <span
        aria-hidden
        className="bg-primary/10 text-primary flex size-8 shrink-0 items-center justify-center rounded-full text-sm font-semibold"
      >
        {initialOf(name)}
      </span>
      <PeerStatusDot online={online} testId={"contact-friend-online-" + friend.peerId} />
      <div className="min-w-0 flex-1">
        {/* P3#15 无昵称时展示的是 PeerId 缩略：hover 显全文并支持复制 */}
        <p
          className="flex items-center gap-1 truncate text-sm font-medium"
          title={friend.nickname ? undefined : friend.peerId}
        >
          {name}
          {friend.nickname ? null : (
            <CopyButton value={friend.peerId} className="size-5 shrink-0" />
          )}
        </p>
        <p className="text-muted-foreground truncate text-xs">
          {friend.note ? friend.note + " · " : ""}
          {friend.group ? friend.group : t("chat.group.ungrouped")}
        </p>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <Button type="button" variant="ghost" size="sm" asChild>
          <Link to={"/chat?peer=" + friend.peerId} data-testid={"contact-friend-message-" + friend.peerId}>
            <MessageSquareIcon aria-hidden className="size-4" />
            {t("contacts.friends.message")}
          </Link>
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => onMove(friend)}
          data-testid={"contact-friend-move-" + friend.peerId}
        >
          <MoveIcon aria-hidden className="size-4" />
          {t("contacts.friends.move")}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => onRemove(friend)}
          data-testid={"contact-friend-remove-" + friend.peerId}
        >
          <Trash2Icon aria-hidden className="size-4" />
          {t("contacts.friends.remove")}
        </Button>
      </div>
    </div>
  );
}
