import { useMemo } from "react";

import type { FriendInviteJson, GroupInviteJson } from "@/lib/ipc-types";
import { useChatStore } from "@/stores/chat-store";

// F17（UX 审计 20260907）：聊天列表的「等待对方同意」占位条目数据源。
// 状态与通讯录收件箱/消息中心同源——直接读 chat-store 既有 invites /
// groupInvites（chat_invites_list / chat_group_invites_list 的镜像），
// 不重复造状态：本文件只做只读投影，无任何 store 写路径。

/** 好友邀请簿本身即待处理集（accepted/rejected 即移出），out=本机等待对方 */
function isWaitingFriendInvite(invite: FriendInviteJson): boolean {
  return invite.direction === "out";
}

/** 群邀请带显式状态机，仅 pending 且本机发出的算等待中 */
function isWaitingGroupInvite(invite: GroupInviteJson): boolean {
  return invite.direction === "out" && invite.state === "pending";
}

export interface PendingInviteItem {
  /** 好友=peerId / 群=groupId，列表 key 与深链 id 同形状 */
  id: string;
  kind: "friend" | "group";
  title: string;
  tsMs: number;
}

/** 纯函数投影（可独立单测）：两路邀请簿 → 置灰占位条目，按发起时间降序 */
export function pendingInviteItems(
  invites: FriendInviteJson[],
  groupInvites: GroupInviteJson[],
): PendingInviteItem[] {
  const friendItems = invites
    .filter(isWaitingFriendInvite)
    .map((invite) => ({
      id: invite.peerId,
      kind: "friend" as const,
      title: invite.nickname || invite.peerId.slice(0, 8),
      tsMs: invite.tsMs,
    }));
  const groupItems = groupInvites
    .filter(isWaitingGroupInvite)
    .map((invite) => ({
      id: invite.groupId,
      kind: "group" as const,
      title: invite.groupName,
      tsMs: invite.tsMs,
    }));
  return [...friendItems, ...groupItems].sort((a, b) => b.tsMs - a.tsMs);
}

export function usePendingInviteItems(): PendingInviteItem[] {
  const invites = useChatStore((s) => s.invites);
  const groupInvites = useChatStore((s) => s.groupInvites);
  return useMemo(
    () => pendingInviteItems(invites, groupInvites),
    [invites, groupInvites],
  );
}
