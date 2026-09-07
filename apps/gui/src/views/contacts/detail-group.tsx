import { useState } from "react";
import { useTranslation } from "react-i18next";
import { MessageSquareIcon, UserPlusIcon, UsersRoundIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { CopyButton } from "@/components/feedback/copy-button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { initialOf } from "@/lib/conversation-entry";
import type { GroupJson } from "@/lib/ipc-types";
import { useGroupStore } from "@/stores/group-store";
import { GroupInvitePicker } from "@/views/group/group-invite-picker";

import { ContactAvatar } from "./contact-avatar";
import {
  DetailAction,
  DetailActionLink,
  DetailActions,
  DetailRow,
  DetailRows,
  DetailShell,
} from "./detail-bits";
import { useGroupLeave } from "./group-leave";

// 群资料卡：头像 + 群名；ID/成员数/角色；底部发消息 / 邀请成员（仅群主）/
// 退群（第三档确认，与行内入口共用 useGroupLeave）。
export function DetailGroup({ group }: { group: GroupJson }) {
  const { t } = useTranslation();
  const selfPeerId = useGroupStore((s) => s.selfPeerId);
  const { leaveGroup, leavingId } = useGroupLeave();
  const [inviteOpen, setInviteOpen] = useState(false);
  const isOwner = selfPeerId !== null && group.owner === selfPeerId;
  return (
    <DetailShell
      avatar={<ContactAvatar initial={initialOf(group.name)} className="size-16 rounded-lg text-xl" />}
      title={<p className="truncate text-lg font-semibold">{group.name}</p>}
    >
      <DetailRows>
        <DetailRow label={t("contacts.detail.peerId")}>
          <span className="font-mono text-xs break-all">{group.groupId}</span>
          <CopyButton value={group.groupId} className="size-5 shrink-0" />
        </DetailRow>
        <DetailRow label={t("contacts.detail.members")}>
          {t("group.members", { count: group.members.length })}
        </DetailRow>
        <DetailRow label={t("contacts.detail.role")}>
          {isOwner ? t("contacts.groups.role.owner") : t("contacts.groups.role.member")}
        </DetailRow>
      </DetailRows>
      <DetailActions>
        <DetailActionLink
          icon={MessageSquareIcon}
          label={t("contacts.groups.message")}
          testId="contacts-detail-message"
          to={"/chat?group=" + group.groupId}
        />
        <DetailAction
          icon={UserPlusIcon}
          label={t("contacts.groups.invite")}
          testId="contacts-detail-invite"
          disabled={!isOwner}
          title={!isOwner ? t("contacts.groups.inviteDisabledNotOwner") : undefined}
          onClick={() => setInviteOpen(true)}
        />
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="text-destructive hover:text-destructive h-auto flex-col gap-1.5 px-3 py-2"
          data-testid="contacts-detail-leave"
          disabled={leavingId === group.groupId}
          onClick={() => void leaveGroup(group)}
        >
          <UsersRoundIcon aria-hidden className="size-5" />
          <span className="text-xs font-normal">{t("contacts.groups.leave")}</span>
        </Button>
      </DetailActions>
      <Dialog open={inviteOpen} onOpenChange={setInviteOpen}>
        <DialogContent className="sm:max-w-md" data-testid="contacts-group-invite-dialog">
          <DialogHeader>
            <DialogTitle>{t("group.manage.inviteTitle")}</DialogTitle>
          </DialogHeader>
          <GroupInvitePicker group={group} onDone={() => setInviteOpen(false)} />
        </DialogContent>
      </Dialog>
    </DetailShell>
  );
}
