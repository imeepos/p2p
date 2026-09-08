import { useState } from "react";
import { useTranslation } from "react-i18next";
import { MessageSquareIcon, MoveIcon, Trash2Icon } from "lucide-react";

import { PeerStatusDot } from "@/components/chat/peer-status";
import { CopyButton } from "@/components/feedback/copy-button";
import { initialOf } from "@/lib/conversation-entry";
import type { ChatFriendJson } from "@/lib/ipc-types";
import { usePeerOnline } from "@/stores/node-store";

import { ChatFriendMoveDialog } from "./chat-friend-move-dialog";
import { ChatFriendRemoveDialog } from "./chat-friend-remove-dialog";
import { ContactAvatar } from "./contact-avatar";
import {
  DetailAction,
  DetailActionLink,
  DetailActions,
  DetailRow,
  DetailRows,
  DetailShell,
} from "./detail-bits";

// 好友资料卡（双栏改版）：头像 + 昵称 + 在线态；ID/备注/分组字段；底部
// 发消息 / 移动分组 / 删除。移动与删除对话框在卡内自持，与行内入口同组件。
export function DetailFriend({ friend }: { friend: ChatFriendJson }) {
  const { t } = useTranslation();
  const online = usePeerOnline(friend.peerId);
  const name = friend.nickname || friend.peerId.slice(0, 8);
  const [moveOpen, setMoveOpen] = useState(false);
  const [removeOpen, setRemoveOpen] = useState(false);
  return (
    <DetailShell
      avatar={<ContactAvatar initial={initialOf(name)} className="size-16 rounded-lg text-xl" />}
      title={
        <p className="flex items-center gap-2 text-lg font-semibold">
          <span className="truncate">{name}</span>
          <PeerStatusDot online={online} testId={"contact-friend-detail-online-" + friend.peerId} />
        </p>
      }
    >
      <DetailRows>
        <DetailRow label={t("contacts.detail.peerId")}>
          <span className="font-mono text-xs break-all">{friend.peerId}</span>
          <CopyButton value={friend.peerId} className="size-5 shrink-0" />
        </DetailRow>
        <DetailRow label={t("contacts.detail.remark")}>
          {friend.note || t("contacts.detail.noRemark")}
        </DetailRow>
        <DetailRow label={t("contacts.detail.group")}>
          {friend.group || t("chat.group.ungrouped")}
        </DetailRow>
      </DetailRows>
      <DetailActions>
        <DetailActionLink
          icon={MessageSquareIcon}
          label={t("contacts.friends.message")}
          testId="contacts-detail-message"
          to={"/chat?peer=" + friend.peerId}
        />
        <DetailAction
          icon={MoveIcon}
          label={t("contacts.friends.move")}
          testId="contacts-detail-move"
          onClick={() => setMoveOpen(true)}
        />
        <DetailAction
          icon={Trash2Icon}
          label={t("contacts.friends.remove")}
          testId="contacts-detail-remove"
          destructive
          onClick={() => setRemoveOpen(true)}
        />
      </DetailActions>
      <ChatFriendMoveDialog
        friend={moveOpen ? friend : null}
        onOpenChange={(open) => !open && setMoveOpen(false)}
      />
      <ChatFriendRemoveDialog
        friend={removeOpen ? friend : null}
        onOpenChange={(open) => !open && setRemoveOpen(false)}
      />
    </DetailShell>
  );
}
