import type { DiscoveredAgent } from "@/a2a/types";
import type { ConversationEntry, BadgeSpec, PresenceTone, PreviewLabels } from "./conversation-entry";
import { initialOf, truncateChars, AGENT_PREVIEW_MAX_CHARS } from "./conversation-entry";

// A2A 会话条目构建器（docs/design/a2a-over-p2p-design.md §8.3）。
// 与 agentEntry 分家：A2A 走 task 语义，不走 ACP transcript 栈。

/** task 状态 → StatusBadge 映射（设计 §8.3）。 */
const TASK_STATE_BADGE: Record<string, BadgeSpec> = {
  submitted: { tone: "secondary", label: "已提交" },
  working: { tone: "secondary", label: "思考中" },
  completed: { tone: "outline", label: "完成" },
  failed: { tone: "destructive", label: "失败" },
  cancelled: { tone: "secondary", label: "已取消" },
  rejected: { tone: "destructive", label: "已拒绝" },
};

/** A2A 会话条目参数。 */
export interface A2aEntryParams {
  agent: DiscoveredAgent;
  lastMessage: string | null;
  lastTsMs: number;
  taskState: string | null;
  unread: number;
  joinSeq: number;
  labels: PreviewLabels;
}

/** 构建 A2A 会话条目（设计 §8.3 聊天集成）。 */
export function a2aEntry(params: A2aEntryParams): ConversationEntry {
  const { agent, lastMessage, lastTsMs, taskState, unread, joinSeq } = params;
  const { card } = agent;
  
  // 在线点启发：剩余 TTL >50% 绿、≤50% 黄、过期灰（gui-contract §17.3）
  const nowSecs = Math.floor(Date.now() / 1000);
  const expiresAt = agent.issuedAtSecs + card.ttlSecs;
  const remainingRatio = agent.issuedAtSecs > 0 
    ? (expiresAt - nowSecs) / card.ttlSecs 
    : 0;
  
  const dot: PresenceTone | null = agent.issuedAtSecs > 0
    ? remainingRatio > 0.5 ? "green" : remainingRatio > 0 ? "yellow" : "red"
    : null;

  const title = card.name;
  const subtitle = card.description ? truncateChars(card.description, 60) : null;
  
  return {
    id: `${card.hostPeer}/${card.agentId}`,
    kind: "a2a",
    title,
    subtitle,
    kindMark: {
      initial: initialOf(title),
      botIcon: true,
      groupBadge: false,
      dot,
    },
    statusBadge: taskState ? TASK_STATE_BADGE[taskState] : null,
    lastPreview: lastMessage 
      ? truncateChars(lastMessage, AGENT_PREVIEW_MAX_CHARS) 
      : null,
    lastTsMs,
    unread,
    sendState: taskState === "working" ? "pending" : 
               taskState === "failed" ? "error" : null,
    host: card.hostPeer,
    joinSeq,
  };
}

/** 从 DiscoveredAgent 数组构建 A2A 会话条目列表。 */
export function a2aEntries(
  agents: DiscoveredAgent[],
  lastMessages: Map<string, { text: string; tsMs: number }>,
  taskStates: Map<string, string>,
  unreadCounts: Record<string, number>,
  labels: PreviewLabels,
): ConversationEntry[] {
  return agents.map((agent, index) => {
    const key = `${agent.card.hostPeer}/${agent.card.agentId}`;
    const lastMsg = lastMessages.get(key);
    return a2aEntry({
      agent,
      lastMessage: lastMsg?.text ?? null,
      lastTsMs: lastMsg?.tsMs ?? 0,
      taskState: taskStates.get(key) ?? null,
      unread: unreadCounts[key] ?? 0,
      joinSeq: index,
      labels,
    });
  });
}