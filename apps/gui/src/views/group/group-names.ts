import type { ChatFriendJson, ChatMessageJson, GroupJson, GroupMessageJson } from "@/lib/ipc-types";

// 群内 PeerId → 展示名：好友簿昵称优先，未在册回退 PeerId 缩略（同 1:1 口径）。
export function groupDisplayName(peerId: string, friends: ChatFriendJson[]): string {
  const friend = friends.find((f) => f.peerId === peerId);
  return friend?.nickname || peerId.slice(0, 8);
}

// 群消息 → 1:1 气泡视图模型：本端判定 senderId === 本机 PeerId（设计 §7），
// me/them 归一后复用 MessageBubble 渲染路径。
export function toBubbleMessage(
  message: GroupMessageJson,
  selfPeerId: string | null,
): ChatMessageJson {
  const isMe = selfPeerId !== null && message.senderId === selfPeerId;
  return {
    id: message.id,
    peer: message.groupId,
    sender: isMe ? "me" : "them",
    kind: message.kind,
    tsMs: message.tsMs,
    text: message.text ?? null,
    media: message.media ?? null,
    status: message.status,
    replyTo: message.replyTo ?? null,
  };
}

// 送达计数的目标数 n（设计 §4「已送达 |acks|/n」）：目标成员 = 名单减本机。
export function groupRecipientTotal(group: { members: string[] }, selfPeerId: string | null): number {
  if (selfPeerId === null) return Math.max(0, group.members.length - 1);
  return group.members.filter((m) => m !== selfPeerId).length;
}

// 展示排序（设计 §7 group_list：GUI 按 state 过滤/置底）：active 按最近
// roster 时间在前，left/kicked/disbanded 整体置底，组内同为时间倒序。
export function orderedGroups(groups: GroupJson[]): GroupJson[] {
  const byRecency = (a: GroupJson, b: GroupJson) => b.tsMs - a.tsMs;
  const active = groups.filter((g) => g.state === "active").sort(byRecency);
  const inactive = groups.filter((g) => g.state !== "active").sort(byRecency);
  return [...active, ...inactive];
}
