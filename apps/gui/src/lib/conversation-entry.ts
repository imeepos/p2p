import type {
  ChatFriendJson,
  ChatKind,
  ChatMessageJson,
  GroupChatState,
  GroupJson,
  GroupMessageJson,
} from "@/lib/ipc-types";

// 会话条目统一模型（docs/design/app-shell-redesign.md §2.2）：单聊/群/agent
// 三来源字段级对齐成一种条目形状，混排排序与搜索过滤也是本文件纯函数。
// 构建器不做 i18n——展示文案经参数传入，保持可独立单测。

export type ConversationKind = "friend" | "group" | "agent";

/** 条目发送状态（§2.2 sendState 行）：friend/group 取本端消息状态机，
 *  agent 仅 pending（等 agent 响应）与 error（连接失败）两态。 */
export type ConversationSendState =
  | "pending"
  | "sent"
  | "delivered"
  | "failed"
  | "error";

/** agent kindMark 连接态色点（§2.2）：绿=在线 黄=连接中/重连中 红=断开 */
export type PresenceTone = "green" | "yellow" | "red";

export interface BadgeSpec {
  tone: "secondary" | "destructive" | "outline";
  label: string;
}

export interface ConversationEntry {
  id: string;
  kind: ConversationKind;
  title: string;
  subtitle: string | null;
  kindMark: {
    /** 头像首字（friend/group）；agent 用 Bot 图标，initial 为 null */
    initial: string | null;
    botIcon: boolean;
    /** 群角标 */
    groupBadge: boolean;
    dot: PresenceTone | null;
  };
  statusBadge: BadgeSpec | null;
  lastPreview: string | null;
  lastTsMs: number;
  unread: number;
  sendState: ConversationSendState | null;
  /** 搜索辅助（§2.4）：agent 额外匹配 wsUrl host；其余来源为 null */
  host: string | null;
  /** 无消息条目按加入序排在有消息之后（§2.2 排序） */
  joinSeq: number;
}

/** 预览构建所需的本地化文案（§2.2 预览规则） */
export interface PreviewLabels {
  image: string;
  audio: string;
  video: string;
  file: string;
  /** 群预览里本端发送者显「我」 */
  self: string;
}

/** 文本预览截断上限：单行心智，超长截断加省略号（行内另有 CSS truncate） */
export const PREVIEW_MAX_CHARS = 80;

/** agent 预览截断上限（§2.2：transcript 最后一条 user/agent 文本截断 40 字） */
export const AGENT_PREVIEW_MAX_CHARS = 40;

export function truncateChars(text: string, max: number): string {
  if (text.length <= max) return text;
  return text.slice(0, max) + "…";
}

/** 正文单行化：换行/连续空白折叠为单空格 */
function singleLine(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}

/** 五种消息 kind 的预览规则（§2.2）：text 正文单行；image/audio/video 类型
 *  词条；file 词条 + 文件名；无消息返回 null。 */
export function previewOfMessage(
  message: Pick<ChatMessageJson, "kind" | "text" | "media"> | null | undefined,
  labels: PreviewLabels,
): string | null {
  if (!message) return null;
  if (message.kind === "text") {
    const text = singleLine(message.text ?? "");
    return text ? truncateChars(text, PREVIEW_MAX_CHARS) : null;
  }
  if (message.kind === "image") return labels.image;
  if (message.kind === "audio") return labels.audio;
  if (message.kind === "video") return labels.video;
  const name = message.media?.name ?? "";
  return truncateChars((labels.file + " " + name).trimEnd(), PREVIEW_MAX_CHARS);
}

/** 头像首字：取首个码点；空标题回退占位符 */
export function initialOf(title: string): string {
  return Array.from(title.trim())[0] ?? "?";
}

export function wsHostOf(wsUrl: string): string | null {
  try {
    return new URL(wsUrl).host || null;
  } catch {
    return null;
  }
}

export function friendEntry(params: {
  friend: ChatFriendJson;
  last: ChatMessageJson | null;
  unread: number;
  joinSeq: number;
  labels: PreviewLabels;
}): ConversationEntry {
  const { friend, last, unread, joinSeq, labels } = params;
  const title = friend.nickname || friend.peerId.slice(0, 8);
  return {
    id: friend.peerId,
    kind: "friend",
    title,
    subtitle: friend.note ?? null,
    kindMark: { initial: initialOf(title), botIcon: false, groupBadge: false, dot: null },
    statusBadge: null,
    lastPreview: previewOfMessage(last, labels),
    lastTsMs: last?.tsMs ?? 0,
    unread,
    // 仅最近一条为本端消息时呈发送状态（§2.2）
    sendState: last && last.sender === "me" ? last.status : null,
    host: null,
    joinSeq,
  };
}

const GROUP_STATE_TONE: Record<
  Exclude<GroupChatState, "active">,
  BadgeSpec["tone"]
> = { left: "secondary", kicked: "destructive", disbanded: "outline" };

export function groupEntry(params: {
  group: GroupJson;
  last: GroupMessageJson | null;
  unread: number;
  selfPeerId: string | null;
  /** 群「N 成员」展示文案 */
  membersLabel: string;
  /** 群状态展示文案；active 时忽略 */
  stateLabel: string;
  /** 发送者 PeerId → 昵称（好友簿缺失回退缩略） */
  nickOf: (peerId: string) => string;
  joinSeq: number;
  labels: PreviewLabels;
}): ConversationEntry {
  const { group, last, unread, selfPeerId, membersLabel, stateLabel, nickOf, joinSeq, labels } =
    params;
  const isMine = last != null && last.senderId === selfPeerId;
  let lastPreview: string | null = null;
  if (last) {
    const sender = isMine ? labels.self : nickOf(last.senderId) || last.senderId.slice(0, 8);
    const body = previewOfMessage(last, labels) ?? "";
    lastPreview = truncateChars(sender + "：" + body, PREVIEW_MAX_CHARS);
  }
  return {
    id: group.groupId,
    kind: "group",
    title: group.name,
    subtitle: membersLabel,
    kindMark: {
      initial: initialOf(group.name),
      botIcon: false,
      groupBadge: true,
      dot: null,
    },
    statusBadge:
      group.state !== "active"
        ? { tone: GROUP_STATE_TONE[group.state], label: stateLabel }
        : null,
    lastPreview,
    lastTsMs: last?.tsMs ?? group.tsMs,
    unread,
    sendState: isMine && last ? last.status : null,
    host: null,
    joinSeq,
  };
}

export type AgentPhase = "idle" | "connecting" | "online" | "reconnecting" | "offline";

export function agentEntry(params: {
  endpointId: string;
  alias: string;
  wsUrl: string;
  phase: AgentPhase;
  /** 连接失败（offline 终态）显 destructive 断连徽标（§2.2） */
  connectFailed: boolean;
  /** 连接态展示文案 */
  connectionLabel: string;
  /** transcript 最后一条 user/agent 文本（可为空） */
  lastText: string | null;
  lastInteractionMs: number;
  unread: number;
  promptPending: boolean;
  joinSeq: number;
  labels: PreviewLabels;
}): ConversationEntry {
  const {
    endpointId, alias, wsUrl, phase, connectFailed, connectionLabel,
    lastText, lastInteractionMs, unread, promptPending, joinSeq,
  } = params;
  const host = wsHostOf(wsUrl);
  const title = alias || host || endpointId;
  const dot: PresenceTone =
    phase === "online"
      ? "green"
      : phase === "connecting" || phase === "reconnecting"
        ? "yellow"
        : "red";
  return {
    id: endpointId,
    kind: "agent",
    title,
    subtitle: connectionLabel,
    kindMark: { initial: null, botIcon: true, groupBadge: false, dot },
    statusBadge: connectFailed ? { tone: "destructive", label: connectionLabel } : null,
    lastPreview: lastText
      ? truncateChars(singleLine(lastText), AGENT_PREVIEW_MAX_CHARS)
      : connectionLabel,
    lastTsMs: lastInteractionMs,
    unread,
    sendState: promptPending ? "pending" : connectFailed ? "error" : null,
    host,
    joinSeq,
  };
}

/** §2.2 排序：lastTsMs 降序；无消息（ts=0）条目按加入序降序排在有消息之后。 */
export function sortEntries(entries: ConversationEntry[]): ConversationEntry[] {
  const withTs = entries
    .filter((e) => e.lastTsMs > 0)
    .sort((a, b) => b.lastTsMs - a.lastTsMs);
  const withoutTs = entries
    .filter((e) => e.lastTsMs <= 0)
    .sort((a, b) => b.joinSeq - a.joinSeq);
  return [...withTs, ...withoutTs];
}

/** §2.4 搜索：title/subtitle 不区分大小写子串；peerId/groupId 前缀；agent
 *  额外匹配 wsUrl host。只过滤不动排序。 */
export function filterEntries(
  entries: ConversationEntry[],
  rawQuery: string,
): ConversationEntry[] {
  const q = rawQuery.trim().toLowerCase();
  if (!q) return entries;
  return entries.filter((e) => {
    if (e.title.toLowerCase().includes(q)) return true;
    if (e.subtitle?.toLowerCase().includes(q)) return true;
    if (e.id.toLowerCase().startsWith(q)) return true;
    if (e.host?.toLowerCase().includes(q)) return true;
    return false;
  });
}

/** §2.3 呈现：≥100 显「99+」 */
export function formatUnreadCount(count: number): string {
  return count >= 100 ? "99+" : String(count);
}
