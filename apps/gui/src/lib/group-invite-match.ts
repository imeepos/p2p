import type { ChatMessageJson, GroupInviteJson } from "@/lib/ipc-types";

// IMC3 卡片 ↔ 邀请条目匹配：1:1 消息体只带群快照，状态与终态以邀请簿为准。
// 优先按会话方向 + 对端 PeerId 精确匹配；事件未达/簿未加载时退化为
// 同方向最新一条（群内多邀请逐条演化，最新条目最接近当前状态）。
export function matchInviteForMessage(
  message: ChatMessageJson,
  invites: GroupInviteJson[],
): GroupInviteJson | null {
  const body = message.groupInvite;
  if (!body) return null;
  const sameGroup = invites.filter((i) => i.groupId === body.groupId);
  if (sameGroup.length === 0) return null;
  const wantDirection = message.sender === "me" ? "out" : "in";
  const counterpartKey = wantDirection === "out" ? "invitee" : "inviter";
  const exact = sameGroup.find(
    (i) => i.direction === wantDirection && i[counterpartKey] === message.peer,
  );
  if (exact) return exact;
  const directional = sameGroup
    .filter((i) => i.direction === wantDirection)
    .sort((a, b) => b.tsMs - a.tsMs);
  return directional[0] ?? null;
}
