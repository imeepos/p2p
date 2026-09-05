import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";
import { MessageSquareIcon, UserPlusIcon, UsersRoundIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useConfirm } from "@/components/feedback/confirm-provider";
import type { GroupJson } from "@/lib/ipc-types";
import { useGroupStore } from "@/stores/group-store";
import { EmptyState } from "@/views/shared/empty-state";

import { GroupStateBadge } from "@/views/group/group-list";
import { GroupCreateDialog } from "@/views/group/group-create-dialog";
import { GroupInvitePicker } from "@/views/group/group-invite-picker";
import { GroupAddDialog } from "./group-add-dialog";

// 群区（§3.1）：行 = 群名 + 成员数 + 我的角色 + 四态徽标；行内操作：
// 发消息（/chat?group=）、邀请成员（owner active）、退群（第三档单次
// 确认）。left/kicked/disbanded 置底沿用 orderedGroups 语义。
export function GroupSection() {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const groups = useGroupStore((s) => s.groups);
  const groupsLoaded = useGroupStore((s) => s.groupsLoaded);
  const selfPeerId = useGroupStore((s) => s.selfPeerId);
  const leave = useGroupStore((s) => s.leave);
  const [addOpen, setAddOpen] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [inviteGroup, setInviteGroup] = useState<GroupJson | null>(null);
  const [commandError, setCommandError] = useState<string | null>(null);

  const leaveGroup = async (group: GroupJson) => {
    const ok = await confirm({
      title: t("contacts.groups.leaveConfirmTitle"),
      description: t("contacts.groups.leaveConfirmDesc", { group: group.name }),
      confirmText: t("contacts.groups.leaveConfirm"),
      cancelText: t("common.actions.cancel"),
      destructive: true,
    });
    if (!ok) return;
    try {
      await leave(group.groupId);
    } catch (error) {
      console.error("[contacts] 退群失败", group.groupId, error);
      setCommandError(
        t("contacts.groups.leaveFailed") +
          (error instanceof Error ? error.message : String(error)),
      );
    }
  };

  const active = (group: GroupJson) => group.state === "active";
  const isOwner = (group: GroupJson) => selfPeerId !== null && group.owner === selfPeerId;
  const sorted = [...groups].sort((a, b) => {
    // active 在前，同态按最近 roster 时间倒序（group-names orderedGroups 同语义）
    if (active(a) !== active(b)) return active(a) ? -1 : 1;
    return b.tsMs - a.tsMs;
  });

  return (
    <section
      id="groups"
      aria-label={t("contacts.section.groups")}
      data-testid="contacts-section-groups"
      className="bg-card ring-border ring-1 flex flex-col gap-2 rounded-lg p-4"
    >
      <div className="flex items-center justify-between gap-2">
        <h2 className="text-sm font-semibold">{t("contacts.section.groups")}</h2>
        <Button type="button" variant="outline" size="sm" onClick={() => setAddOpen(true)} data-testid="contacts-group-add">
          <UsersRoundIcon aria-hidden className="size-4" />
          {t("contacts.groups.add")}
        </Button>
      </div>

      {commandError ? (
        <p className="text-destructive text-xs" role="alert" data-testid="contacts-group-error">
          {commandError}
        </p>
      ) : null}

      {!groupsLoaded && groups.length === 0 ? (
        <p className="text-muted-foreground px-1 py-2 text-sm">{t("group.loading")}</p>
      ) : sorted.length === 0 ? (
        <EmptyState
          icon={UsersRoundIcon}
          title={t("contacts.groups.empty")}
          description={t("contacts.groups.emptyHint")}
          action={
            <Button type="button" variant="outline" size="sm" onClick={() => setAddOpen(true)}>
              {t("contacts.groups.add")}
            </Button>
          }
        />
      ) : (
        sorted.map((group) => (
          <div
            key={group.groupId}
            className="hover:bg-accent/50 flex items-center gap-3 rounded-md px-2 py-2"
            data-testid={"contact-group-" + group.groupId}
          >
            <span
              aria-hidden
              className="bg-primary/10 text-primary flex size-8 shrink-0 items-center justify-center rounded-full text-sm font-semibold"
            >
              {Array.from(group.name)[0] ?? "?"}
            </span>
            <div className="min-w-0 flex-1">
              <p className="flex items-center gap-2 truncate text-sm font-medium">
                {group.name}
                {group.state !== "active" ? <GroupStateBadge state={group.state} /> : null}
              </p>
              <p className="text-muted-foreground truncate text-xs">
                {t("group.members", { count: group.members.length })} ·{" "}
                {isOwner(group) ? t("contacts.groups.role.owner") : t("contacts.groups.role.member")}
              </p>
            </div>
            <div className="flex shrink-0 items-center gap-1">
              <Button type="button" variant="ghost" size="sm" asChild>
                <Link to={"/chat?group=" + group.groupId} data-testid={"contact-group-message-" + group.groupId}>
                  <MessageSquareIcon aria-hidden className="size-4" />
                  {t("contacts.groups.message")}
                </Link>
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                disabled={!isOwner(group) || !active(group)}
                onClick={() => setInviteGroup(group)}
                data-testid={"contact-group-invite-" + group.groupId}
              >
                <UserPlusIcon aria-hidden className="size-4" />
                {t("contacts.groups.invite")}
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                disabled={!active(group)}
                onClick={() => void leaveGroup(group)}
                data-testid={"contact-group-leave-" + group.groupId}
              >
                {t("contacts.groups.leave")}
              </Button>
            </div>
          </div>
        ))
      )}

      <GroupAddDialog
        open={addOpen}
        onOpenChange={setAddOpen}
        onCreate={() => setCreateOpen(true)}
      />
      <GroupCreateDialog open={createOpen} onOpenChange={setCreateOpen} />
      <Dialog open={inviteGroup !== null} onOpenChange={(next) => !next && setInviteGroup(null)}>
        <DialogContent className="sm:max-w-md" data-testid="contacts-group-invite-dialog">
          <DialogHeader>
            <DialogTitle>{t("group.manage.inviteTitle")}</DialogTitle>
          </DialogHeader>
          {inviteGroup ? (
            <GroupInvitePicker
              group={inviteGroup}
              onDone={() => setInviteGroup(null)}
            />
          ) : null}
        </DialogContent>
      </Dialog>
    </section>
  );
}
