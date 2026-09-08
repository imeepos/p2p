import { useMemo } from "react";
import { useTranslation } from "react-i18next";

import { useAcpStore } from "@/acp/acp-store";
import { lastTurnText } from "@/acp/agent-summary";
import type { AcpPhase } from "@/acp/protocol";
import {
  agentEntry,
  friendEntry,
  groupEntry,
  visibleGroups,
  type ConversationEntry,
  type PreviewLabels,
} from "@/lib/conversation-entry";
import { a2aEntries } from "@/lib/conversation-entry-a2a";
import type { A2aTaskState } from "@/a2a/task-types";
import { applyConversationPrefs } from "@/lib/conversation-overlay";
import { useChatStore } from "@/stores/chat-store";
import { useConversationPrefsStore } from "@/stores/conversation-prefs-store";
import { useGroupStore } from "@/stores/group-store";
import { useUiPrefsStore } from "@/stores/ui-prefs-store";
import { useA2aStore } from "@/a2a/a2a-store";
import { useAgentsStore } from "@/a2a/agents-store";

// §2.2 store 层聚合：四来源构建统一条目并混排排序，渲染层无来源分支。
// agent 为单连接语义：仅 activeEndpointId 继承全局连接态，其余端点显未连接。

const GROUP_STATE_KEYS = {
  left: "group.state.left",
  kicked: "group.state.kicked",
  disbanded: "group.state.disbanded",
} as const;

const PHASE_KEYS = {
  idle: "acp.connection.phase.idle",
  connecting: "acp.connection.phase.connecting",
  online: "acp.connection.phase.online",
  reconnecting: "acp.connection.phase.reconnecting",
  offline: "acp.connection.phase.offline",
} as const;

export function useConversationEntries(): ConversationEntry[] {
  const { t } = useTranslation();
  const friends = useChatStore((s) => s.friends);
  const lastMessageByPeer = useChatStore((s) => s.lastMessageByPeer);
  const unreadByPeer = useChatStore((s) => s.unreadByPeer);
  const groups = useGroupStore((s) => s.groups);
  const groupFriends = useGroupStore((s) => s.friends);
  const lastMessageByGroup = useGroupStore((s) => s.lastMessageByGroup);
  const unreadByGroup = useGroupStore((s) => s.unreadByGroup);
  const selfPeerId = useGroupStore((s) => s.selfPeerId);
  const saved = useAcpStore((s) => s.saved);
  const phase = useAcpStore((s) => s.phase);
  const activeEndpointId = useAcpStore((s) => s.activeEndpointId);
  const activeSessionId = useAcpStore((s) => s.activeSessionId);
  const transcripts = useAcpStore((s) => s.transcripts);
  const lastInteractionByEndpoint = useAcpStore((s) => s.lastInteractionByEndpoint);
  const unreadByEndpoint = useAcpStore((s) => s.unreadByEndpoint);
  const promptPendingBySession = useAcpStore((s) => s.promptPendingBySession);
  const showInactiveGroups = useUiPrefsStore((s) => s.showInactiveGroups);
  const convFlags = useConversationPrefsStore((s) => s.flags);
  const dismissedAt = useConversationPrefsStore((s) => s.dismissedAt);
  const discoveredAgents = useAgentsStore((s) => s.discovered);
  const a2aTasks = useA2aStore((s) => s.tasks);
  const unreadByAgent = useA2aStore((s) => s.unreadByAgent);

  return useMemo(() => {
    const labels: PreviewLabels = {
      image: t("chat.preview.image"),
      audio: t("chat.preview.audio"),
      video: t("chat.preview.video"),
      file: t("chat.preview.file"),
      self: t("chat.conversations.self"),
      groupInvite: t("chat.preview.groupInvite"),
    };
    const nickOf = (peerId: string) =>
      groupFriends.find((f) => f.peerId === peerId)?.nickname ?? "";

    const friendEntries = friends.map((friend, index) =>
      friendEntry({
        friend,
        last: lastMessageByPeer[friend.peerId] ?? null,
        unread: unreadByPeer[friend.peerId] ?? 0,
        joinSeq: index,
        labels,
      }),
    );
    const groupEntries = visibleGroups(groups, showInactiveGroups).map((group, index) =>
      groupEntry({
        group,
        last: lastMessageByGroup[group.groupId] ?? null,
        unread: unreadByGroup[group.groupId] ?? 0,
        selfPeerId,
        membersLabel: t("group.members", { count: group.members.length }),
        stateLabel:
          group.state === "active" ? "" : t(GROUP_STATE_KEYS[group.state]),
        nickOf,
        joinSeq: index,
        labels,
      }),
    );
    const agentEntries = saved.map((endpoint, index) => {
      const id = endpoint.endpointId ?? endpoint.wsUrl;
      const isActive = id === activeEndpointId;
      const entryPhase: AcpPhase = isActive ? phase : "idle";
      return agentEntry({
        endpointId: id,
        alias: endpoint.alias ?? "",
        wsUrl: endpoint.wsUrl,
        phase: entryPhase,
        connectFailed: isActive && phase === "offline",
        connectionLabel: t(PHASE_KEYS[entryPhase]),
        lastText: isActive ? lastTurnText(transcripts, activeSessionId) : null,
        lastInteractionMs: lastInteractionByEndpoint[id] ?? 0,
        unread: unreadByEndpoint[id] ?? 0,
        promptPending: isActive
          ? (activeSessionId !== null && (promptPendingBySession[activeSessionId] ?? false))
          : false,
        joinSeq: index,
        labels,
      });
    });

    // A2A 条目：从 discovered agents 和 tasks 构建
    const lastMessages = new Map<string, { text: string; tsMs: number }>();
    const taskStates = new Map<string, A2aTaskState>();
    for (const [, task] of a2aTasks) {
      const agentKey = task.agentId;
      const lastMsg = task.messages[task.messages.length - 1];
      if (lastMsg) {
        const textPart = lastMsg.parts.find((p) => p.type === "text");
        if (textPart && textPart.type === "text") {
          lastMessages.set(agentKey, { text: textPart.text, tsMs: Date.now() });
        }
      }
      taskStates.set(agentKey, task.state);
    }

    const a2aEntriesList = a2aEntries(
      discoveredAgents,
      lastMessages,
      taskStates,
      unreadByAgent,
      labels,
    );

    // 右键菜单偏好在列表层统一覆盖：删除/不显示过滤、标为未读抬底、置顶分区
    return applyConversationPrefs(
      [...friendEntries, ...groupEntries, ...agentEntries, ...a2aEntriesList],
      { flags: convFlags, dismissedAt },
    );
  }, [
    t, friends, lastMessageByPeer, unreadByPeer, groups, groupFriends,
    lastMessageByGroup, unreadByGroup, selfPeerId, saved, phase,
    activeEndpointId, activeSessionId, transcripts, lastInteractionByEndpoint,
    unreadByEndpoint, promptPendingBySession, showInactiveGroups,
    convFlags, dismissedAt, discoveredAgents, a2aTasks, unreadByAgent,
  ]);
}