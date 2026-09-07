import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDownIcon, ChevronRightIcon, UserPlusIcon } from "lucide-react";
import { useSearchParams } from "react-router-dom";

import { AsyncButton } from "@/components/feedback/async-button";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/views/shared/empty-state";
import { errorText } from "@/views/shared/form-flow";
import type { ChatFriendJson, FriendInviteJson } from "@/lib/ipc-types";
import { useChatStore } from "@/stores/chat-store";

import { collapseKeyOf, groupSections, loadCollapsedGroups, saveCollapsedGroups } from "./chat-friend-group";
import { ChatFriendMoveDialog } from "./chat-friend-move-dialog";
import { ChatFriendRemoveDialog } from "./chat-friend-remove-dialog";
import { ChatFriendAddDialog } from "./chat-friend-add-dialog";
import { FriendRow } from "./friend-row";
import { matchesQuery } from "./contacts-sections";
import { SectionHeader } from "./section-search";

// 好友区（§3.1）：分组折叠交互随迁移保留（不再是页面级结构）；顶部
// out 邀请「待对方同意」条目可撤回；节头「添加」开加好友对话框。
export function FriendSection() {
  const { t } = useTranslation();
  const friends = useChatStore((s) => s.friends);
  const invites = useChatStore((s) => s.invites) ?? [];
  const friendsError = useChatStore((s) => s.friendsError);
  const loadFriends = useChatStore((s) => s.loadFriends);
  const cancelInvite = useChatStore((s) => s.cancelInvite);
  // 跨卡 URL 契约：#/contacts?add=<peerId> 挂载即开添加好友弹窗并预填；
  // 弹窗关闭时清掉参数，避免重挂载重复弹出。
  const [searchParams, setSearchParams] = useSearchParams();
  const addParam = searchParams.get("add");
  const [addOpen, setAddOpen] = useState(() => addParam !== null);
  const [addSeed] = useState(() => addParam ?? "");

  const handleAddOpenChange = (open: boolean) => {
    setAddOpen(open);
    if (!open && searchParams.get("add") !== null) setSearchParams({});
  };
  const [moveTarget, setMoveTarget] = useState<ChatFriendJson | null>(null);
  const [removeTarget, setRemoveTarget] = useState<ChatFriendJson | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(() => loadCollapsedGroups());
  // P2#8 检索：昵称/备注/PeerId 子串匹配，大小写不敏感（节内 state）
  const [query, setQuery] = useState("");

  const toggleGroup = (name: string | null) => {
    setCollapsed((prev) => {
      const key = collapseKeyOf(name);
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      saveCollapsedGroups(next);
      return next;
    });
  };

  const outgoing = invites.filter((i: FriendInviteJson) => i.direction === "out");
  const filtered = friends.filter((f) => matchesQuery([f.nickname, f.note, f.peerId], query));

  return (
    <section
      id="friends"
      aria-label={t("contacts.section.friends")}
      data-testid="contacts-section-friends"
      className="bg-card ring-border ring-1 flex flex-col gap-2 rounded-lg p-4"
    >
      <SectionHeader
        id="friends"
        title={t("contacts.section.friends")}
        query={query}
        onQueryChange={setQuery}
        placeholder={t("contacts.friends.searchPlaceholder")}
        matched={filtered.length}
        total={friends.length}
        addLabel={t("contacts.friends.add")}
        addIcon={UserPlusIcon}
        onAdd={() => setAddOpen(true)}
        addTestId="contacts-friend-add"
      />

      {outgoing.map((invite) => (
        <div
          key={invite.peerId}
          className="flex items-center gap-2 rounded-md border border-dashed px-2 py-1.5"
          data-testid={"contacts-invite-out-" + invite.peerId}
        >
          <span className="text-muted-foreground text-xs">
            {t("chat.invite.outgoing", { name: invite.nickname })} · {t("contacts.friends.pendingOut")}
          </span>
          <AsyncButton
            type="button"
            variant="ghost"
            size="sm"
            className="ml-auto"
            action={() => cancelInvite(invite.peerId)}
            onSuccess={() => toastSuccess(t("contacts.friends.cancelInviteSuccess"))}
            onError={(error) =>
              toastError(t("contacts.friends.cancelInviteFailed"), {
                description: errorText(error),
                context: "chat_friend_cancel",
              })
            }
            data-testid={"contacts-invite-cancel-" + invite.peerId}
          >
            {t("contacts.friends.cancelInvite")}
          </AsyncButton>
        </div>
      ))}

      {friendsError ? (
        <div className="flex flex-col gap-1" data-testid="contacts-friends-error">
          <p className="text-destructive text-sm">{t("contacts.friends.loadFailed")}</p>
          <p className="text-muted-foreground text-xs">{friendsError}</p>
          <Button type="button" variant="outline" size="sm" className="w-fit" onClick={() => void loadFriends()}>
            {t("chat.retry")}
          </Button>
        </div>
      ) : friends.length === 0 && outgoing.length === 0 ? (
        <EmptyState
          icon={UserPlusIcon}
          title={t("contacts.friends.empty")}
          description={t("contacts.friends.emptyHint")}
          action={
            <Button type="button" variant="outline" size="sm" onClick={() => setAddOpen(true)}>
              {t("contacts.friends.add")}
            </Button>
          }
        />
      ) : filtered.length === 0 && friends.length > 0 ? (
        <p className="text-muted-foreground px-1 py-2 text-sm" data-testid="contacts-friends-no-match">
          {t("contacts.noMatch")}
        </p>
      ) : (
        groupSections(filtered).map((section) => {
          const key = collapseKeyOf(section.name);
          const isCollapsed = collapsed.has(key);
          return (
            <div key={key ?? "__ungrouped__"} className="flex flex-col gap-0.5">
              <button
                type="button"
                className="text-muted-foreground hover:text-foreground flex w-fit items-center gap-1 px-1 py-1 text-xs font-medium"
                aria-expanded={!isCollapsed}
                onClick={() => toggleGroup(section.name)}
                data-testid={"contacts-friend-group-" + key}
              >
                {isCollapsed ? (
                  <ChevronRightIcon aria-hidden className="size-3.5" />
                ) : (
                  <ChevronDownIcon aria-hidden className="size-3.5" />
                )}
                {section.name ?? t("chat.group.ungrouped")}（{section.friends.length}）
              </button>
              {!isCollapsed
                ? section.friends.map((friend) => (
                    <FriendRow
                      key={friend.peerId}
                      friend={friend}
                      onMove={setMoveTarget}
                      onRemove={setRemoveTarget}
                    />
                  ))
                : null}
            </div>
          );
        })
      )}

      <ChatFriendAddDialog
        open={addOpen}
        onOpenChange={handleAddOpenChange}
        initialPeerId={addSeed}
      />
      <ChatFriendMoveDialog friend={moveTarget} onOpenChange={(open) => !open && setMoveTarget(null)} />
      <ChatFriendRemoveDialog friend={removeTarget} onOpenChange={(open) => !open && setRemoveTarget(null)} />
    </section>
  );
}