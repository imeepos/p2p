import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { ArrowLeft, MessageSquare } from "lucide-react";

import { ConversationList } from "@/components/chat/conversation-list";
import { useAcpStore } from "@/acp/acp-store";
import { Button } from "@/components/ui/button";
import { useConversationEntries } from "@/views/chat/use-conversation-entries";
import type { ConversationEntry } from "@/lib/conversation-entry";
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";
import { EmptyState } from "@/views/shared/empty-state";
import { NARROW_CHAT_QUERY, useMediaQuery } from "@/hooks/use-media-query";

import { AgentConversation } from "./agent-conversation";
import { FriendConversation } from "./friend-conversation";
import { GroupMemberPanel } from "@/views/group/group-member-panel";
import { GroupConversation } from "@/views/group/group-conversation";

// /chat 双栏统一会话页（docs/design/app-shell-redesign.md §2）：
// - 左侧通栏会话列表（顶部搜索框）+ 右侧会话记录；宽度 xl:320px / 其余 264px。
// - 选中态路由化 ?peer=/?group=/?agent=；无 query 右侧空态（§2.1）。
// - <768 单栏互斥（防御性规则）：默认显列表，选中切入记录，记录左上返回。
// - ?kind=group|agent（旧 /group /acp 重定向落点，拍板项 1）：聚焦排序最前
//   的对应条目，无对应条目保持空态。
const SELECTION_KEYS = ["kind", "peer", "group", "agent"] as const;

export function ChatPage() {
  const { t } = useTranslation();
  const [searchParams, setSearchParams] = useSearchParams();
  const narrow = useMediaQuery(NARROW_CHAT_QUERY);
  const entries = useConversationEntries();
  const friendsLoaded = useChatStore((s) => s.friendsLoaded);
  const friendsError = useChatStore((s) => s.friendsError);
  const loadFriends = useChatStore((s) => s.loadFriends);
  const loadInvites = useChatStore((s) => s.loadInvites);
  const selectPeer = useChatStore((s) => s.selectPeer);
  const subscribeEvents = useChatStore((s) => s.subscribeEvents);
  const groups = useGroupStore((s) => s.groups);
  const groupsLoaded = useGroupStore((s) => s.groupsLoaded);
  const loadGroups = useGroupStore((s) => s.loadGroups);
  const refreshSelf = useGroupStore((s) => s.refreshSelf);
  const ensureFriends = useGroupStore((s) => s.ensureFriends);
  const selectGroup = useGroupStore((s) => s.selectGroup);
  const subscribeGroupEvents = useGroupStore((s) => s.subscribeEvents);
  const [manageOpen, setManageOpen] = useState(false);

  useEffect(() => {
    void loadFriends();
    void loadInvites();
    void subscribeEvents();
    void loadGroups();
    void refreshSelf();
    void ensureFriends();
    void subscribeGroupEvents();
  }, [loadFriends, loadInvites, subscribeEvents, loadGroups, refreshSelf, ensureFriends, subscribeGroupEvents]);

  const peerParam = searchParams.get("peer");
  const groupParam = searchParams.get("group");
  const agentParam = searchParams.get("agent");
  const kindParam = searchParams.get("kind");
  const selectedId = peerParam ?? groupParam ?? agentParam;

  // 深链落定：选中即清零（1:1/群在 select* 内清；agent 经聚焦入口清，§2.3）。
  // 离开 agent 会话（切走/卸载）即取消聚焦，agent 回复恢复计未读。
  useEffect(() => {
    if (!agentParam) return;
    useAcpStore.getState().setFocusedEndpoint(agentParam);
    return () => useAcpStore.getState().setFocusedEndpoint(null);
  }, [agentParam]);
  useEffect(() => {
    if (!peerParam) return;
    // 深链历史加载失败：historyError 已入 store 供 UI 呈现，此处留观测日志
    selectPeer(peerParam).catch((error) => {
      console.error("[chat] 深链会话历史加载失败", peerParam, error);
    });
  }, [peerParam, selectPeer]);
  useEffect(() => {
    if (!groupParam) return;
    selectGroup(groupParam).catch((error) => {
      console.error("[chat] 深链群历史加载失败", groupParam, error);
    });
  }, [groupParam, selectGroup]);

  // ?kind=* 聚焦（拍板项 1）：无显式选中时落排序最前的对应条目
  useEffect(() => {
    if (peerParam || groupParam || agentParam) return;
    if (kindParam !== "group" && kindParam !== "agent") return;
    const first = entries.find((e) => e.kind === kindParam);
    if (!first) return;
    const key = first.kind === "group" ? "group" : "agent";
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.delete("kind");
        next.set(key, first.id);
        return next;
      },
      { replace: true },
    );
  }, [kindParam, peerParam, groupParam, agentParam, entries, setSearchParams]);

  const selectEntry = (entry: ConversationEntry) => {
    const key = entry.kind === "friend" ? "peer" : entry.kind === "group" ? "group" : "agent";
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        for (const k of SELECTION_KEYS) next.delete(k);
        next.set(key, entry.id);
        return next;
      },
      { replace: true },
    );
  };
  const clearSelection = () => setSearchParams(new URLSearchParams(), { replace: true });

  // §2.1 单栏互斥：窄屏下列表与记录互斥呈现；宽屏双栏并存
  const showList = !narrow || !selectedId;
  const showConversation = !narrow || !!selectedId;
  const listLoading = !friendsLoaded && !friendsError && !groupsLoaded;
  const group = groupParam ? (groups.find((g) => g.groupId === groupParam) ?? null) : null;

  return (
    <div data-testid="chat-page" className="flex min-h-0 flex-1 gap-0">
      {showList ? (
        <section
          aria-label={t("chat.conversations.searchPlaceholder")}
          data-testid="chat-list-pane"
          className="flex min-h-0 w-[264px] shrink-0 flex-col rounded-lg border xl:w-[320px]"
        >
          <ConversationList
            entries={entries}
            selectedId={selectedId}
            loading={listLoading}
            error={friendsError}
            onRetry={async () => {
              await loadFriends();
              const err = useChatStore.getState().friendsError;
              if (err) throw new Error(err);
            }}
            onSelect={selectEntry}
          />
        </section>
      ) : null}
      {showConversation ? (
        <section
          aria-label={t("chat.conversation")}
          data-testid="chat-conversation-pane"
          className="flex min-h-0 min-w-0 flex-1 flex-col rounded-lg border"
        >
          {narrow && selectedId ? (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="mr-auto"
              data-testid="chat-back"
              onClick={clearSelection}
            >
              <ArrowLeft aria-hidden />
              {t("chat.conversations.back")}
            </Button>
          ) : null}
          {peerParam ? (
            <FriendConversation key={peerParam} peer={peerParam} />
          ) : group && groupParam ? (
            <GroupConversation
              key={group.groupId}
              group={group}
              onOpenManage={() => setManageOpen(true)}
            />
          ) : agentParam ? (
            <AgentConversation endpointId={agentParam} />
          ) : (
            <EmptyState
              className="max-w-none flex-1"
              icon={MessageSquare}
              title={t("chat.conversations.empty")}
              description={t("chat.conversations.emptyHint")}
            />
          )}
        </section>
      ) : null}
      {group && manageOpen ? (
        <GroupMemberPanel group={group} open onOpenChange={setManageOpen} />
      ) : null}
    </div>
  );
}