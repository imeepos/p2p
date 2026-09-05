import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { useConfirm } from "@/components/feedback/confirm-provider";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import type { GroupJson } from "@/lib/ipc-types";
import { useGroupStore } from "@/stores/group-store";

import { groupDisplayName } from "./group-names";
import { GroupInvitePicker } from "./group-invite-picker";

interface GroupMemberPanelProps {
  group: GroupJson;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

// 成员面板（G3）：成员列表（昵称/群主/我）、owner 工具（邀请/移除/改名/
// 解散）、成员退群。确认流统一走 useConfirm；命令失败原文上浮不静默。
export function GroupMemberPanel({ group, open, onOpenChange }: GroupMemberPanelProps) {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const selfPeerId = useGroupStore((s) => s.selfPeerId);
  const friends = useGroupStore((s) => s.friends);
  const ensureFriends = useGroupStore((s) => s.ensureFriends);
  const kick = useGroupStore((s) => s.kick);
  const leave = useGroupStore((s) => s.leave);
  const rename = useGroupStore((s) => s.rename);
  const disband = useGroupStore((s) => s.disband);
  const [inviting, setInviting] = useState(false);
  // 面板由 GroupView 按需挂载（open 才 mount），初值即打开时刻的群名，
  // 无需 effect 内同步 setState 重置。
  const [name, setName] = useState(group.name);
  const [busy, setBusy] = useState(false);
  const [commandError, setCommandError] = useState<string | null>(null);

  useEffect(() => {
    void ensureFriends();
  }, [ensureFriends]);

  const isOwner = selfPeerId !== null && group.owner === selfPeerId;
  const active = group.state === "active";

  const run = (
    failKey:
      | "group.manage.kickFailed"
      | "group.manage.leaveFailed"
      | "group.manage.disbandFailed"
      | "group.manage.renameFailed",
    action: () => Promise<unknown>,
  ) => {
    if (busy) return Promise.resolve();
    setBusy(true);
    setCommandError(null);
    return action()
      .catch((error) => {
        console.error("[group] 成员面板操作失败", error);
        const reason = error instanceof Error ? error.message : String(error);
        setCommandError(t(failKey) + reason);
      })
      .finally(() => setBusy(false));
  };

  const kickMember = async (member: string) => {
    const ok = await confirm({
      title: t("group.manage.kickConfirmTitle"),
      description: t("group.manage.kickConfirmDesc", {
        name: groupDisplayName(member, friends),
        group: group.name,
      }),
      confirmText: t("group.manage.kickConfirm"),
      destructive: true,
    });
    if (ok) void run("group.manage.kickFailed", () => kick(group.groupId, member));
  };

  const leaveGroup = async () => {
    const ok = await confirm({
      title: t("group.manage.leaveConfirmTitle"),
      description: t("group.manage.leaveConfirmDesc", { group: group.name }),
      confirmText: t("group.manage.leaveConfirm"),
      destructive: true,
    });
    if (ok) void run("group.manage.leaveFailed", () => leave(group.groupId));
  };

  // 解散（G6 起 owner-only 真命令）：rev+1，对全体其他成员 G_KICK(disbanded)，
  // 本端 state=disbanded；重复解散后端显式 Err 原文上浮。
  const disbandGroup = async () => {
    const ok = await confirm({
      title: t("group.manage.disbandConfirmTitle"),
      description: t("group.manage.disbandConfirmDesc", { group: group.name }),
      confirmText: t("group.manage.disbandConfirm"),
      destructive: true,
    });
    if (!ok) return;
    void run("group.manage.disbandFailed", () => disband(group.groupId));
  };

  const renameGroup = () => {
    const trimmed = name.trim();
    if (!trimmed || trimmed === group.name) return;
    void run("group.manage.renameFailed", () => rename(group.groupId, trimmed));
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg" data-testid="group-member-panel">
        <DialogHeader>
          <DialogTitle>{t("group.manage.title")}</DialogTitle>
          <DialogDescription>
            {t("group.members", { count: group.members.length })} ·{" "}
            {t("group.ownerLabel")} {groupDisplayName(group.owner, friends)}
          </DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <div className="flex flex-col gap-1" data-testid="group-member-list">
            {group.members.map((member) => (
              <div
                key={member}
                className="flex items-center gap-2 rounded px-2 py-1 text-sm"
                data-testid={`group-member-${member}`}
              >
                <span className="truncate">{groupDisplayName(member, friends)}</span>
                {member === group.owner ? (
                  <span className="text-muted-foreground text-xs">
                    {t("group.manage.ownerBadge")}
                  </span>
                ) : null}
                {member === selfPeerId ? (
                  <span className="text-muted-foreground text-xs">
                    {t("group.manage.meBadge")}
                  </span>
                ) : null}
                {isOwner && active && member !== group.owner ? (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="ml-auto"
                    disabled={busy}
                    data-testid={`group-kick-${member}`}
                    onClick={() => void kickMember(member)}
                  >
                    {t("group.manage.kickAction")}
                  </Button>
                ) : null}
              </div>
            ))}
          </div>
          {commandError ? (
            <p className="text-destructive text-xs" role="alert" data-testid="group-panel-error">
              {commandError}
            </p>
          ) : null}
        </div>
        <MemberPanelFooter
          group={group}
          isOwner={isOwner}
          active={active}
          busy={busy}
          inviting={inviting}
          name={name}
          onNameChange={setName}
          onRename={renameGroup}
          onToggleInvite={() => setInviting((v) => !v)}
          onLeave={() => void leaveGroup()}
          onDisband={() => void disbandGroup()}
        />
      </DialogContent>
    </Dialog>
  );
}

interface FooterProps {
  group: GroupJson;
  isOwner: boolean;
  active: boolean;
  busy: boolean;
  inviting: boolean;
  name: string;
  onNameChange: (name: string) => void;
  onRename: () => void;
  onToggleInvite: () => void;
  onLeave: () => void;
  onDisband: () => void;
}

function MemberPanelFooter({
  group,
  isOwner,
  active,
  busy,
  inviting,
  name,
  onNameChange,
  onRename,
  onToggleInvite,
  onLeave,
  onDisband,
}: FooterProps) {
  const { t } = useTranslation();
  if (!isOwner && !active) {
    return (
      <p className="text-xs text-muted-foreground" data-testid="group-panel-readonly">
        {t("group.readOnlyHint", { state: t(`group.state.${group.state}`) })}
      </p>
    );
  }
  return (
    <div className="flex flex-col gap-3">
      {isOwner && active && inviting ? (
        <GroupInvitePicker group={group} onDone={onToggleInvite} />
      ) : null}
      {isOwner && active ? (
        <div className="flex items-center gap-2">
          <Input
            value={name}
            onChange={(event) => onNameChange(event.target.value)}
            aria-label={t("group.manage.renameLabel")}
            data-testid="group-rename-input"
            autoComplete="off"
            className="flex-1"
          />
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={busy || name.trim() === group.name}
            onClick={onRename}
            data-testid="group-rename-save"
          >
            {t("group.manage.renameSubmit")}
          </Button>
        </div>
      ) : null}
      <div className="flex items-center gap-2">
        {isOwner && active ? (
          <>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={busy}
              data-testid="group-invite-open"
              onClick={onToggleInvite}
            >
              {t("group.manage.inviteAction")}
            </Button>
            <Button
              type="button"
              variant="destructive"
              size="sm"
              disabled={busy}
              data-testid="group-disband"
              onClick={onDisband}
            >
              {t("group.manage.disbandAction")}
            </Button>
          </>
        ) : null}
        {!isOwner && active ? (
          <Button
            type="button"
            variant="destructive"
            size="sm"
            disabled={busy}
            data-testid="group-leave"
            onClick={onLeave}
          >
            {t("group.manage.leaveAction")}
          </Button>
        ) : null}
      </div>
    </div>
  );
}
