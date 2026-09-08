import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { ArrowLeft } from "lucide-react";

import { ConversationList } from "@/components/chat/conversation-list";
import { useAcpStore } from "@/acp/acp-store";
import { Button } from "@/components/ui/button";
import { useComposePrefillStore } from "@/stores/compose-prefill-store";
import { useConversationEntries } from "@/views/chat/use-conversation-entries";
import { usePendingInviteItems } from "@/views/chat/use-pending-invites";
import { ChatEmptyState } from "@/views/chat/chat-empty-state";
import type { ConversationEntry } from "@/lib/conversation-entry";
import { useChatStore } from "@/stores/chat-store";
import { conversationKey, useConversationPrefsStore } from "@/stores/conversation-prefs-store";
import { useGroupStore } from "@/stores/group-store";
import { NARROW_CHAT_QUERY, useMediaQuery } from "@/hooks/use-media-query";

import { AgentConversation } from "./agent-conversation";
import { InactiveGroupsToggle } from "./inactive-groups-toggle";
import { FriendConversation } from "./friend-conversation";
import { GroupPendingPanel } from "./group-pending-panel";
import { GroupMemberPanel } from "@/views/group/group-member-panel";
import { GroupConversation } from "@/views/group/group-conversation";
import { A2aConversation } from "./a2a-conversation";

// /chat 双栏统一会话页（docs/design/app-shell-redesign.md §2）：
// - 左侧通栏会话列表（顶部搜索框）+ 右侧会话记录；宽度 xl:320px / 其余 264px。
// - 选中态路由化 ?peer=/?group=/?agent=；无 query 右侧空态（§2.1）。
// - <768 单栏互斥（防御性规则）：默认显列表，选中切入记录，记录左上返回。
// - ?kind=group|agent（旧 /group /acp 重定向落点，拍板项 1）：聚焦排序最前
//   的对应条目，无对应条目保持空态。
const SELECTION_KEYS = ["kind", "peer", "group", "agent", "a2a"] as const;

export function ChatPage() {
  const { t } = useTranslation();
  const [searchParams, setSearchParams] = useSearchParams();
  const narrow = useMediaQuery(NARROW_CHAT_QUERY);
  const entries = useConversationEntries();
  const pendingInvites = usePendingInviteItems();
  const friendsLoaded = useChatStore((s) => s.friendsLoaded);
  const friendsError = useChatStore((s) => s.friendsError);
  const loadFriends = useChatStore((s) => s.loadFriends);
  const loadInvites = useChatStore((s) => s.loadInvites);
  const loadGroupInvites = useChatStore((s) => s.loadGroupInvites);
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
  const setManualUnread = useConversationPrefsStore((s) => s.setManualUnread);
  // W4：?compose= 深链预填（分享链接「发送到聊天」落点），消费即清防重放
  const composeParam = searchParams.get("compose");

  useEffect(() => {
    if (!composeParam) return;
    useComposePrefillStore.getState().setText(composeParam);
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.delete("compose");
        return next;
      },
      { replace: true },
    );
  }, [composeParam, setSearchParams]);

  useEffect(() => {
    void loadFriends();
    void loadInvites();
    void loadGroupInvites();
    void subscribeEvents();
    void loadGroups();
    void refreshSelf();
    void ensureFriends();
    void subscribeGroupEvents();
  }, [loadFriends, loadInvites, loadGroupInvites, subscribeEvents, loadGroups, refreshSelf, ensureFriends, subscribeGroupEvents]);

  const peerParam = searchParams.get("peer");
  const groupParam = searchParams.get("group");
  const agentParam = searchParams.get("agent");
  const a2aParam = searchParams.get("a2a");
  const kindParam = searchParams.get("kind");
  const selectedId = peerParam ?? groupParam ?? agentParam ?? a2aParam;

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
    if (peerParam || groupParam || agentParam || a2aParam) return;
    if (kindParam !== "group" && kindParam !== "agent" && kindParam !== "a2a") return;
    const first = entries.find((e) => e.kind === kindParam);
    if (!first) return;
    const key = first.kind === "group" ? "group" : 
                first.kind === "a2a" ? "a2a" : "agent";
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.delete("kind");
        next.set(key, first.id);
        return next;
      },
      { replace: true },
    );
  }, [kindParam, peerParam, groupParam, agentParam, a2aParam, entries, setSearchParams]);

  const selectEntry = (entry: ConversationEntry) => {
    // 打开即已读：清「标为未读」旗标（store 未读由 selectPeer/selectGroup 清零）
    setManualUnread(conversationKey(entry.kind, entry.id), false);
    const key = entry.kind === "friend" ? "peer" : 
                entry.kind === "group" ? "group" : 
                entry.kind === "a2a" ? "a2a" : "agent";
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
  // 已退出/已解散群聊计数：驱动侧栏底部显隐开关（默认隐藏，见 ui-prefs-store）
  const inactiveGroupCount = groups.filter((g) => g.state !== "active").length;

  return (
    <div data-testid="chat-page" className="flex min-h-0 flex-1 gap-0">
      {showList ? (
        <section
          aria-label={t("chat.conversations.searchPlaceholder")}
          data-testid="chat-list-pane"
          className="bg-wx-list flex min-h-0 w-[264px] shrink-0 flex-col border-r border-border/60 xl:w-[320px]"
        >
          <ConversationList
            entries={entries}
            selectedId={selectedId}
            loading={listLoading}
            error={friendsError}
            pendingInvites={pendingInvites}
            onRetry={async () => {
              await loadFriends();
              const err = useChatStore.getState().friendsError;
              if (err) throw new Error(err);
            }}
            onSelect={selectEntry}
          />
          <InactiveGroupsToggle hiddenCount={inactiveGroupCount} />
        </section>
      ) : null}
      {showConversation ? (
        <section
          aria-label={t("chat.conversation")}
          data-testid="chat-conversation-pane"
          className="bg-wx-chat flex min-h-0 min-w-0 flex-1 flex-col"
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
          ) : groupParam ? (
            // IMC3：同意入群后 roster 未达的跳转时序兜底（加载态→自动进入）；
            // key=groupId 换群即重挂载复位超时窗口
            <GroupPendingPanel key={groupParam} groupId={groupParam} />
          ) : agentParam ? (
            <AgentConversation endpointId={agentParam} />
          ) : a2aParam ? (
            <A2aConversation
              key={a2aParam}
              agentKey={a2aParam}
              hostPeer={a2aParam.split("/")[0] || ""}
              agentId={a2aParam.split("/")[1] || ""}
              agentName={entries.find((e) => e.id === a2aParam)?.title || a2aParam}
            />
          ) : (
            <ChatEmptyState
              friendCount={entries.filter((e) => e.kind === "friend").length}
              groupCount={entries.filter((e) => e.kind === "group").length}
              agentCount={entries.filter((e) => e.kind === "agent").length}
              a2aCount={entries.filter((e) => e.kind === "a2a").length}
              pendingInviteCount={pendingInvites.length}
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