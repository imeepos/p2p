import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { MessageSquareIcon, PencilLineIcon, Trash2Icon } from "lucide-react";

import { PeerStatusDot } from "@/components/chat/peer-status";
import { CopyButton } from "@/components/feedback/copy-button";
import { initialOf } from "@/lib/conversation-entry";
import type { ChatFriendJson } from "@/lib/ipc-types";
import { usePeerOnline } from "@/stores/node-store";
import { usePeerProfileStore } from "@/stores/peer-profile-store";

import { ChatFriendEditDialog } from "./chat-friend-edit-dialog";
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

// 好友资料卡（双栏改版）：头像 + 昵称 + 在线态；ID/备注字段（备注可编辑）；
// 有对端自报资料时头像用对方头像、简介行展示对方简介（/im/profile/1）；
// 底部 发消息 / 编辑资料 / 删除。编辑与删除对话框在卡内自持。
export function DetailFriend({ friend }: { friend: ChatFriendJson }) {
  const { t } = useTranslation();
  const online = usePeerOnline(friend.peerId);
  const fetchProfile = usePeerProfileStore((s) => s.fetch);
  const peerProfile = usePeerProfileStore((s) => s.profiles[friend.peerId] ?? null);
  const name = friend.nickname || friend.peerId.slice(0, 8);
  const [removeOpen, setRemoveOpen] = useState(false);
  const [editOpen, setEditOpen] = useState(false);

  useEffect(() => {
    void fetchProfile(friend.peerId);
  }, [friend.peerId, fetchProfile]);

  return (
    <DetailShell
      avatar={
        <ContactAvatar
          initial={initialOf(name)}
          src={peerProfile?.avatar ?? null}
          className="size-16 rounded-lg text-xl"
        />
      }
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
        <DetailRow label={t("contacts.detail.intro")}>
          {peerProfile?.description ? (
            <span
              className="text-muted-foreground min-w-0 flex-1 text-sm break-words"
              data-testid="contacts-detail-intro"
            >
              {peerProfile.description}
            </span>
          ) : (
            <span className="text-muted-foreground text-sm" data-testid="contacts-detail-intro">
              {t("contacts.detail.noIntro")}
            </span>
          )}
        </DetailRow>
        <DetailRow label={t("contacts.detail.remark")}>
          <button
            type="button"
            className="min-w-0 flex-1 truncate text-left hover:underline"
            onClick={() => setEditOpen(true)}
            data-testid="contacts-detail-remark-edit"
            title={t("contacts.friends.editTitle")}
          >
            {friend.note || t("contacts.detail.noRemark")}
          </button>
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
          icon={PencilLineIcon}
          label={t("contacts.friends.edit")}
          testId="contacts-detail-edit"
          onClick={() => setEditOpen(true)}
        />
        <DetailAction
          icon={Trash2Icon}
          label={t("contacts.friends.remove")}
          testId="contacts-detail-remove"
          destructive
          onClick={() => setRemoveOpen(true)}
        />
      </DetailActions>
      {editOpen ? (
        <ChatFriendEditDialog friend={friend} onOpenChange={(open) => !open && setEditOpen(false)} />
      ) : null}
      <ChatFriendRemoveDialog
        friend={removeOpen ? friend : null}
        onOpenChange={(open) => !open && setRemoveOpen(false)}
      />
    </DetailShell>
  );
}
