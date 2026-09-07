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
import { visibleGroups, initialOf } from "@/lib/conversation-entry";
import type { GroupJson } from "@/lib/ipc-types";
import { useGroupStore } from "@/stores/group-store";
import { EmptyState } from "@/views/shared/empty-state";
import { cn } from "@/lib/utils";
import { GroupInvitePicker } from "@/views/group/group-invite-picker";

import { CONTACT_ROW_CLS, ContactAvatar, ROW_ACTIONS_CLS } from "./contact-avatar";
import { selectionKey } from "./contacts-detail-model";
import { matchesQuery, useContactsPane } from "./contacts-sections";
import { TreeSection } from "./contacts-tree";
import { GroupAddDialog } from "./group-add-dialog";
import { useGroupLeave } from "./group-leave";

// 群行（双栏改版）：点选联动右栏资料卡；行内动作（发消息/邀请成员/退群）
// 悬停显隐，邀请与退群仍由节内对话框/确认承接。
function GroupRow(props: {
  group: GroupJson;
  isOwner: boolean;
  leaving: boolean;
  onInvite: (group: GroupJson) => void;
  onLeave: (group: GroupJson) => void;
}) {
  const { t } = useTranslation();
  const pane = useContactsPane();
  const { group, isOwner, leaving, onInvite, onLeave } = props;
  const selected = pane.selectedKey === selectionKey({ kind: "group", groupId: group.groupId });
  return (
    <div
      className={cn(CONTACT_ROW_CLS, selected ? "bg-accent" : "hover:bg-accent/60")}
      data-testid={"contact-group-" + group.groupId}
    >
      <button
        type="button"
        className="flex min-w-0 flex-1 items-center gap-2.5 rounded-md text-left"
        onClick={() => pane.select({ kind: "group", groupId: group.groupId })}
      >
        <ContactAvatar initial={initialOf(group.name)} />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm font-medium">{group.name}</span>
          <span className="text-muted-foreground block truncate text-xs">
            {t("group.members", { count: group.members.length })} ·{" "}
            {isOwner ? t("contacts.groups.role.owner") : t("contacts.groups.role.member")}
          </span>
        </span>
      </button>
      <div className={ROW_ACTIONS_CLS}>
        <Button type="button" variant="ghost" size="icon" className="size-7" asChild>
          <Link
            to={"/chat?group=" + group.groupId}
            data-testid={"contact-group-message-" + group.groupId}
            title={t("contacts.groups.message")}
            aria-label={t("contacts.groups.message")}
          >
            <MessageSquareIcon aria-hidden className="size-4" />
          </Link>
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-7"
          disabled={!isOwner}
          title={!isOwner ? t("contacts.groups.inviteDisabledNotOwner") : t("contacts.groups.invite")}
          aria-label={t("contacts.groups.invite")}
          onClick={() => onInvite(group)}
          data-testid={"contact-group-invite-" + group.groupId}
        >
          <UserPlusIcon aria-hidden className="size-4" />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-7"
          disabled={leaving}
          onClick={() => onLeave(group)}
          data-testid={"contact-group-leave-" + group.groupId}
          title={t("contacts.groups.leave")}
          aria-label={t("contacts.groups.leave")}
        >
          <UsersRoundIcon aria-hidden className="size-4" />
        </Button>
      </div>
    </div>
  );
}

// 群节（§3.1，双栏改版）：只列在群（active），已退出/已解散/被踢不进通讯
// 录；检索走全局 Context 词。F09 起建群表单内嵌 GroupAddDialog 默认页签。
export function GroupSection() {
  const { t } = useTranslation();
  const pane = useContactsPane();
  const groups = useGroupStore((s) => s.groups);
  const groupsLoaded = useGroupStore((s) => s.groupsLoaded);
  const selfPeerId = useGroupStore((s) => s.selfPeerId);
  const { leaveGroup, leavingId } = useGroupLeave();
  const [addOpen, setAddOpen] = useState(false);
  const [inviteGroup, setInviteGroup] = useState<GroupJson | null>(null);

  const isOwner = (group: GroupJson) => selfPeerId !== null && group.owner === selfPeerId;
  // 非 active 群不进通讯录：复用会话列表同款可见性（默认仅 active），同态按
  // 最近 roster 时间倒序（group-names orderedGroups 同语义）
  const listed = visibleGroups(groups, false).sort((a, b) => b.tsMs - a.tsMs);
  const filtered = listed.filter((g) => matchesQuery([g.name, g.groupId], pane.query));

  return (
    <TreeSection
      id="groups"
      wrapperTestId="contacts-section-groups"
      title={t("contacts.section.groups")}
      expanded={!pane.isSectionCollapsed("groups")}
      onToggle={() => pane.toggleSection("groups")}
      toggleTestId="contacts-tree-toggle-groups"
      active={pane.activeSection === "groups"}
      onGo={() => pane.gotoSection("groups")}
      anchorTestId="contacts-anchor-groups"
      anchorLabel={t("contacts.anchor.goto", { section: t("contacts.section.groups") })}
      count={
        <span className="text-muted-foreground shrink-0 text-xs" data-testid="contacts-count-groups">
          {t("contacts.countOf", { matched: filtered.length, total: listed.length })}
        </span>
      }
      actions={
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-6"
          onClick={() => setAddOpen(true)}
          data-testid="contacts-group-add"
          title={t("contacts.groups.add")}
          aria-label={t("contacts.groups.add")}
        >
          <UsersRoundIcon aria-hidden className="size-4" />
        </Button>
      }
    >
      {!groupsLoaded && groups.length === 0 ? (
        <p className="text-muted-foreground px-1 py-2 text-sm">{t("group.loading")}</p>
      ) : listed.length === 0 ? (
        <EmptyState
          icon={UsersRoundIcon}
          title={t("contacts.groups.empty")}
          description={t("contacts.groups.emptyHint")}
        />
      ) : filtered.length === 0 ? (
        <p className="text-muted-foreground px-1 py-2 text-sm" data-testid="contacts-groups-no-match">
          {t("contacts.noMatch")}
        </p>
      ) : (
        filtered.map((group) => (
          <GroupRow
            key={group.groupId}
            group={group}
            isOwner={isOwner(group)}
            leaving={leavingId === group.groupId}
            onInvite={setInviteGroup}
            onLeave={(g) => void leaveGroup(g)}
          />
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
    </TreeSection>
  );
}
