import { useState } from "react";
import { useTranslation } from "react-i18next";
import { UserPlusIcon, UserRoundPlusIcon } from "lucide-react";
import { useSearchParams } from "react-router-dom";

import { AsyncButton } from "@/components/feedback/async-button";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/views/shared/empty-state";
import { errorText } from "@/views/shared/form-flow";
import type { ChatFriendJson, FriendInviteJson } from "@/lib/ipc-types";
import { useChatStore } from "@/stores/chat-store";

import { ChatFriendRemoveDialog } from "./chat-friend-remove-dialog";
import { ChatFriendAddDialog } from "./chat-friend-add-dialog";
import { CONTACT_ROW_CLS, ContactAvatar } from "./contact-avatar";
import { matchesQuery, useContactsPane } from "./contacts-sections";
import { TreeSection } from "./contacts-tree";
import { FriendRow } from "./friend-row";

// 好友节（§3.1，双栏改版）：树分节锚点 + 计数；检索走全局 Context 词；
// out 邀请「待对方同意」条目可撤回；行内动作悬停显隐。好友平铺不分组。
export function FriendSection() {
  const { t } = useTranslation();
  const pane = useContactsPane();
  const friends = useChatStore((s) => s.friends);
  const invites = useChatStore((s) => s.invites) ?? [];
  const friendsError = useChatStore((s) => s.friendsError);
  const loadFriends = useChatStore((s) => s.loadFriends);
  const cancelInvite = useChatStore((s) => s.cancelInvite);
  // 跨卡 URL 契约：#/contacts?add=<peerId> 开添加好友弹窗并预填；弹窗
  // 关闭时清掉参数，避免重挂载重复弹出。参数后到（左栏按钮）也响应。
  const [searchParams, setSearchParams] = useSearchParams();
  const addParam = searchParams.get("add");
  const [addOpen, setAddOpen] = useState(() => addParam !== null);
  const [addSeed, setAddSeed] = useState(() => addParam ?? "");
  const [lastAddParam, setLastAddParam] = useState(addParam);
  if (addParam !== lastAddParam) {
    setLastAddParam(addParam);
    if (addParam !== null) {
      setAddOpen(true);
      setAddSeed(addParam);
    }
  }

  const handleAddOpenChange = (open: boolean) => {
    setAddOpen(open);
    if (!open && searchParams.get("add") !== null) setSearchParams({});
  };
  const [removeTarget, setRemoveTarget] = useState<ChatFriendJson | null>(null);

  const outgoing = invites.filter(
    (i: FriendInviteJson) => i.direction === "out" && matchesQuery([i.nickname, i.peerId], pane.query),
  );
  const filtered = friends.filter((f) => matchesQuery([f.nickname, f.note, f.peerId], pane.query));
  const expanded = !pane.isSectionCollapsed("friends");
  const rowCls = CONTACT_ROW_CLS + " hover:bg-accent/60";

  return (
    <TreeSection
      id="friends"
      wrapperTestId="contacts-section-friends"
      title={t("contacts.section.friends")}
      expanded={expanded}
      onToggle={() => pane.toggleSection("friends")}
      toggleTestId="contacts-tree-toggle-friends"
      active={pane.activeSection === "friends"}
      onGo={() => pane.gotoSection("friends")}
      anchorTestId="contacts-anchor-friends"
      anchorLabel={t("contacts.anchor.goto", { section: t("contacts.section.friends") })}
      count={
        <span className="text-muted-foreground shrink-0 text-xs" data-testid="contacts-count-friends">
          {t("contacts.countOf", { matched: filtered.length, total: friends.length })}
        </span>
      }
      actions={
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-6"
          onClick={() => setAddOpen(true)}
          data-testid="contacts-friend-add-inline"
          title={t("contacts.friends.add")}
          aria-label={t("contacts.friends.add")}
        >
          <UserPlusIcon aria-hidden className="size-4" />
        </Button>
      }
    >
      {outgoing.map((invite) => (
        <div
          key={invite.peerId}
          className={rowCls}
          data-testid={"contacts-invite-out-" + invite.peerId}
        >
          <ContactAvatar initial={invite.nickname.slice(0, 1) || "?"} />
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-medium">{invite.nickname}</p>
            <p className="text-muted-foreground truncate text-xs">{t("contacts.friends.pendingOut")}</p>
          </div>
          <AsyncButton
            type="button"
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-xs"
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
          icon={UserRoundPlusIcon}
          title={t("contacts.friends.empty")}
          description={t("contacts.friends.emptyHint")}
        />
      ) : filtered.length === 0 && friends.length > 0 ? (
        <p className="text-muted-foreground px-1 py-2 text-sm" data-testid="contacts-friends-no-match">
          {t("contacts.noMatch")}
        </p>
      ) : (
        filtered.map((friend) => (
          <FriendRow key={friend.peerId} friend={friend} onRemove={setRemoveTarget} />
        ))
      )}

      <ChatFriendAddDialog
        open={addOpen}
        onOpenChange={handleAddOpenChange}
        initialPeerId={addSeed}
      />
      <ChatFriendRemoveDialog friend={removeTarget} onOpenChange={(open) => !open && setRemoveTarget(null)} />
    </TreeSection>
  );
}
