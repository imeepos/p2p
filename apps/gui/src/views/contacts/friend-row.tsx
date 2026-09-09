import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";
import { MessageSquareIcon, Trash2Icon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { CopyButton } from "@/components/feedback/copy-button";
import { cn, } from "@/lib/utils";
import { initialOf } from "@/lib/conversation-entry";
import type { ChatFriendJson } from "@/lib/ipc-types";
import { usePeerOnline } from "@/stores/node-store";
import { PeerStatusDot } from "@/components/chat/peer-status";

import { CONTACT_ROW_CLS, ContactAvatar, ROW_ACTIONS_CLS } from "./contact-avatar";
import { selectionKey } from "./contacts-detail-model";
import { useContactsPane } from "./contacts-sections";
import { FriendRoleBadge } from "./friend-role-badge";

interface FriendRowProps {
  friend: ChatFriendJson;
  onRemove: (friend: ChatFriendJson) => void;
}

// 好友行（§3.1，双栏改版）：圆角方首字头像 + 昵称 + 备注 + 在线点；
// 点选行切换右栏资料卡；行内动作（发消息/删除）悬停显隐，
// 删除仍由分区自持对话框承接（与资料卡入口同组件）。
export function FriendRow({ friend, onRemove }: FriendRowProps) {
  const { t } = useTranslation();
  const pane = useContactsPane();
  const online = usePeerOnline(friend.peerId);
  const name = friend.nickname || friend.peerId.slice(0, 8);
  const selected = pane.selectedKey === selectionKey({ kind: "friend", peerId: friend.peerId });
  return (
    <div
      className={cn(CONTACT_ROW_CLS, selected ? "bg-accent" : "hover:bg-accent/60")}
      data-testid={"contact-friend-" + friend.peerId}
    >
      <button
        type="button"
        className="flex min-w-0 flex-1 items-center gap-2.5 rounded-md text-left"
        onClick={() => pane.select({ kind: "friend", peerId: friend.peerId })}
        title={friend.nickname ? undefined : friend.peerId}
      >
        <ContactAvatar initial={initialOf(name)} />
        <span className="min-w-0 flex-1">
          <span className="flex items-center gap-1.5 text-sm font-medium">
            <span className="truncate">{name}</span>
            <FriendRoleBadge peerId={friend.peerId} />
            <PeerStatusDot online={online} testId={"contact-friend-online-" + friend.peerId} />
          </span>
          <span className="text-muted-foreground block truncate text-xs">
            {friend.note ? friend.note : ""}
          </span>
        </span>
      </button>
      {friend.nickname ? null : <CopyButton value={friend.peerId} className="size-5 shrink-0" />}
      <div className={ROW_ACTIONS_CLS}>
        <Button type="button" variant="ghost" size="icon" className="size-7" asChild>
          <Link
            to={"/chat?peer=" + friend.peerId}
            data-testid={"contact-friend-message-" + friend.peerId}
            title={t("contacts.friends.message")}
            aria-label={t("contacts.friends.message")}
          >
            <MessageSquareIcon aria-hidden className="size-4" />
          </Link>
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-7"
          onClick={() => onRemove(friend)}
          data-testid={"contact-friend-remove-" + friend.peerId}
          title={t("contacts.friends.remove")}
          aria-label={t("contacts.friends.remove")}
        >
          <Trash2Icon aria-hidden className="size-4" />
        </Button>
      </div>
    </div>
  );
}
