import { ChevronDown, MessageCircle, UserPlusIcon } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import {
  collapseKeyOf,
  groupSections,
  loadCollapsedGroups,
  saveCollapsedGroups,
} from "@/components/chat/chat-friend-group";
import { FriendRow } from "@/components/chat/friend-row";
import { AsyncButton } from "@/components/feedback/async-button";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { ChatFriendJson, ChatMessageJson, GroupChatState, GroupJson } from "@/lib/ipc-types";
import { cn } from "@/lib/utils";
import { EmptyState } from "@/views/shared/empty-state";
import { orderedGroups } from "@/views/group/group-names";

interface ConversationListProps {
  friends: ChatFriendJson[];
  lastMessages: Record<string, ChatMessageJson | null>;
  selectedPeer: string | null;
  loading: boolean;
  error: string | null;
  onSelect: (peerId: string) => void;
  onAddFriend?: () => void;
  onMoveFriend?: (peerId: string) => void;
  onRemoveFriend?: (peerId: string) => void;
  /** 加载失败重试（store 侧 loadFriends；失败时抛错以驱动按钮失败态） */
  onRetry?: () => Promise<void>;
  /** 群会话混排（G3）：传入即渲染群聊分节（active 在前，非 active 置底） */
  groups?: GroupJson[];
  selectedGroupId?: string | null;
  onSelectGroup?: (groupId: string) => void;
}

function summaryOf(message: ChatMessageJson | null | undefined): string | null {
  if (!message) return null;
  if (message.kind === "text") return message.text ?? null;
  return message.media?.name ?? null;
}

const GROUP_STATE_VARIANT: Record<
  Exclude<GroupChatState, "active">,
  "secondary" | "destructive" | "outline"
> = {
  left: "secondary",
  kicked: "destructive",
  disbanded: "outline",
};

// 群会话行（G3 混排）：群名 + 成员数；非 active 附状态徽标，点击跳群聊页。
function GroupMixRow({
  group,
  isActive,
  onSelect,
}: {
  group: GroupJson;
  isActive: boolean;
  onSelect: (groupId: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <li>
      <button
        type="button"
        onClick={() => onSelect(group.groupId)}
        data-testid={`chat-group-row-${group.groupId}`}
        className={cn(
          "w-full px-3 py-2 text-left text-sm hover:bg-accent",
          isActive && "bg-accent",
        )}
      >
        <div className="flex items-center gap-2">
          <span className="truncate font-medium">{group.name}</span>
          {group.state !== "active" ? (
            <Badge variant={GROUP_STATE_VARIANT[group.state]}>
              {t(`group.state.${group.state}`)}
            </Badge>
          ) : null}
        </div>
        <div className="text-muted-foreground text-xs">
          {t("group.members", { count: group.members.length })}
        </div>
      </button>
    </li>
  );
}

// 群聊分节（混排）：恒置 1:1 分组之后；组内 active 在前、非 active 置底。
function GroupMixSection({
  groups,
  selectedGroupId,
  onSelect,
}: {
  groups: GroupJson[];
  selectedGroupId: string | null | undefined;
  onSelect: (groupId: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <section data-testid="chat-group-section">
      <div className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-muted-foreground">
        <span className="truncate font-medium">{t("group.title")}</span>
        <span className="ml-auto tabular-nums">{groups.length}</span>
      </div>
      <ul className="divide-y">
        {orderedGroups(groups).map((group) => (
          <GroupMixRow
            key={group.groupId}
            group={group}
            isActive={group.groupId === selectedGroupId}
            onSelect={onSelect}
          />
        ))}
      </ul>
    </section>
  );
}

// 组内按最后消息时间倒序（无消息排末尾），跨组顺序由 groupSections 决定。
function sortedByRecency(
  friends: ChatFriendJson[],
  lastMessages: Record<string, ChatMessageJson | null>,
): ChatFriendJson[] {
  return [...friends].sort((a, b) => {
    const ta = lastMessages[a.peerId]?.tsMs ?? 0;
    const tb = lastMessages[b.peerId]?.tsMs ?? 0;
    return tb - ta;
  });
}

// 会话列表（IM-T43 分组渲染）：组名字典序分节，未分组虚拟组恒置底；
// 组头可折叠，折叠态持久 localStorage（损坏回退全展开）；组内按时间倒序。
export function ConversationList({
  friends,
  lastMessages,
  selectedPeer,
  loading,
  error,
  onSelect,
  onAddFriend,
  onMoveFriend,
  onRemoveFriend,
  onRetry,
  groups,
  selectedGroupId,
  onSelectGroup,
}: ConversationListProps) {
  const { t } = useTranslation();
  const [collapsed, setCollapsed] = useState<Set<string>>(
    () => loadCollapsedGroups(),
  );

  if (loading) {
    return <p className="p-4 text-sm text-muted-foreground">{t("chat.friendsLoading")}</p>;
  }
  if (error) {
    return (
      <div className="flex flex-col items-start gap-2 p-4">
        <p className="text-destructive text-sm">{error}</p>
        {onRetry ? (
          <AsyncButton
            type="button"
            size="sm"
            variant="outline"
            action={onRetry}
            onError={(retryError) => {
              console.error("[chat] 好友列表重试失败", retryError);
            }}
          >
            {t("common.actions.refresh")}
          </AsyncButton>
        ) : null}
      </div>
    );
  }
  if (friends.length === 0 && (!groups || groups.length === 0)) {
    return (
      <EmptyState
        className="min-h-56 flex-1"
        icon={MessageCircle}
        title={t("chat.noFriends")}
        description={t("chat.noFriendsHint")}
        action={
          onAddFriend ? (
            <Button
              type="button"
              onClick={onAddFriend}
              data-testid="chat-add-friend-empty"
            >
              <UserPlusIcon aria-hidden />
              {t("chat.addFriend.action")}
            </Button>
          ) : undefined
        }
      />
    );
  }

  const toggleGroup = (name: string | null) => {
    setCollapsed((prev) => {
      const key = collapseKeyOf(name);
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      saveCollapsedGroups(next);
      return next;
    });
  };

  return (
    <div>
      {groupSections(friends).map((section) => {
        const key = collapseKeyOf(section.name);
        const isCollapsed = collapsed.has(key);
        return (
          <section key={key} data-testid={`friend-group-${key}`}>
            <button
              type="button"
              aria-expanded={!isCollapsed}
              data-testid={`friend-group-header-${key}`}
              onClick={() => toggleGroup(section.name)}
              className="flex w-full items-center gap-1.5 px-3 py-1.5 text-xs text-muted-foreground hover:bg-accent"
            >
              <ChevronDown
                aria-hidden
                className={cn(
                  "size-3.5 transition-transform",
                  isCollapsed && "-rotate-90",
                )}
              />
              <span className="truncate font-medium">
                {section.name ?? t("chat.group.ungrouped")}
              </span>
              <span className="ml-auto tabular-nums">
                {section.friends.length}
              </span>
            </button>
            {!isCollapsed ? (
              <ul className="divide-y">
                {sortedByRecency(section.friends, lastMessages).map((friend) => (
                  <FriendRow
                    key={friend.peerId}
                    friend={friend}
                    last={lastMessages[friend.peerId] ?? null}
                    summary={summaryOf(lastMessages[friend.peerId])}
                    isActive={selectedPeer === friend.peerId}
                    onSelect={onSelect}
                    onMoveFriend={onMoveFriend}
                    onRemoveFriend={onRemoveFriend}
                  />
                ))}
              </ul>
            ) : null}
          </section>
        );
      })}
      {groups && groups.length > 0 && onSelectGroup ? (
        <GroupMixSection
          groups={groups}
          selectedGroupId={selectedGroupId}
          onSelect={onSelectGroup}
        />
      ) : null}
    </div>
  );
}
