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
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { visibleGroups } from "@/lib/conversation-entry";
import type { GroupJson } from "@/lib/ipc-types";
import { useGroupStore } from "@/stores/group-store";
import { EmptyState } from "@/views/shared/empty-state";
import { errorText } from "@/views/shared/form-flow";

import { GroupInvitePicker } from "@/views/group/group-invite-picker";
import { GroupAddDialog } from "./group-add-dialog";
import { matchesQuery } from "./contacts-sections";
import { SectionHeader } from "./section-search";
// F09：建群表单内嵌 GroupAddDialog 默认页签，不再单独挂 GroupCreateDialog。

// 群区（§3.1）：只列在群（active），已退出/已解散/被踢不进通讯录（退群
// 后行即消失；历史回看走 chat 侧 inactive 开关与 /group 管理页）。行内
// 操作：发消息（/chat?group=）、邀请成员（owner）、退群（第三档单次确认）。
export function GroupSection() {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const groups = useGroupStore((s) => s.groups);
  const groupsLoaded = useGroupStore((s) => s.groupsLoaded);
  const selfPeerId = useGroupStore((s) => s.selfPeerId);
  const leave = useGroupStore((s) => s.leave);
  const [addOpen, setAddOpen] = useState(false);
  const [inviteGroup, setInviteGroup] = useState<GroupJson | null>(null);
  // P2#8 检索：群名/群 ID 子串匹配，大小写不敏感（节内 state）
  const [query, setQuery] = useState("");
  // 退群在途按群锁定，防连点重复发 IPC
  const [leavingId, setLeavingId] = useState<string | null>(null);

  const leaveGroup = async (group: GroupJson) => {
    const ok = await confirm({
      title: t("contacts.groups.leaveConfirmTitle"),
      description: t("contacts.groups.leaveConfirmDesc", { group: group.name }),
      confirmText: t("contacts.groups.leaveConfirm"),
      cancelText: t("common.actions.cancel"),
      destructive: true,
    });
    if (!ok) return;
    setLeavingId(group.groupId);
    try {
      await leave(group.groupId);
      toastSuccess(t("contacts.groups.leaveSuccess"));
    } catch (error) {
      console.error("[contacts] 退群失败", group.groupId, error);
      toastError(t("contacts.groups.leaveFailed"), {
        description: errorText(error),
        context: "group_leave",
      });
    } finally {
      setLeavingId(null);
    }
  };

  const isOwner = (group: GroupJson) => selfPeerId !== null && group.owner === selfPeerId;
  // 非 active 群不进通讯录：复用会话列表同款可见性（默认仅 active），同态按
  // 最近 roster 时间倒序（group-names orderedGroups 同语义）
  const listed = visibleGroups(groups, false).sort((a, b) => b.tsMs - a.tsMs);
  const filtered = listed.filter((g) => matchesQuery([g.name, g.groupId], query));

  return (
    <section
      id="groups"
      aria-label={t("contacts.section.groups")}
      data-testid="contacts-section-groups"
      className="bg-card ring-border ring-1 flex flex-col gap-2 rounded-lg p-4"
    >
      <SectionHeader
        id="groups"
        title={t("contacts.section.groups")}
        query={query}
        onQueryChange={setQuery}
        placeholder={t("contacts.groups.searchPlaceholder")}
        matched={filtered.length}
        total={listed.length}
        addLabel={t("contacts.groups.add")}
        addIcon={UsersRoundIcon}
        onAdd={() => setAddOpen(true)}
        addTestId="contacts-group-add"
      />

      {!groupsLoaded && groups.length === 0 ? (
        <p className="text-muted-foreground px-1 py-2 text-sm">{t("group.loading")}</p>
      ) : listed.length === 0 ? (
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
      ) : filtered.length === 0 ? (
        <p className="text-muted-foreground px-1 py-2 text-sm" data-testid="contacts-groups-no-match">
          {t("contacts.noMatch")}
        </p>
      ) : (
        filtered.map((group) => (
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
              <p className="truncate text-sm font-medium">{group.name}</p>
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
                disabled={!isOwner(group)}
                title={!isOwner(group) ? t("contacts.groups.inviteDisabledNotOwner") : undefined}
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
                disabled={leavingId === group.groupId}
                onClick={() => void leaveGroup(group)}
                data-testid={"contact-group-leave-" + group.groupId}
              >
                {t("contacts.groups.leave")}
              </Button>
            </div>
          </div>
        ))
      )}

      <GroupAddDialog open={addOpen} onOpenChange={setAddOpen} />
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
