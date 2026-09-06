import { ipc } from "@/lib/ipc";
import type { GroupInviteJson, GroupInviteState } from "@/lib/ipc-types";

// chat-store 群邀请切片（IMC3）：初次 list 加载 + chat_group_invite 事件
// 增量 upsert + 状态迁移幂等（终态不回退、重复事件不重复入列）。
// 切片经组合并入 chat-store（chat-store.ts 行数红线），好友邀请沿用既有切片。

export interface GroupInviteSlice {
  groupInvites: GroupInviteJson[];
  groupInvitesLoaded: boolean;
  /** 初次 list 加载失败信号（消息中心页上浮原文），成功清除 */
  groupInvitesError: string | null;
  loadGroupInvites: () => Promise<void>;
  /** chat_group_invite 事件入口：幂等合并，无变化不触发订阅通知 */
  upsertGroupInvite: (invite: GroupInviteJson) => void;
  sendGroupInvite: (
    groupId: string,
    peerId: string,
    note: string | null,
  ) => Promise<GroupInviteJson>;
  acceptGroupInvite: (inviteId: string) => Promise<void>;
  rejectGroupInvite: (inviteId: string, reason: string | null) => Promise<void>;
}

type SliceSet = (
  partial:
    | Partial<GroupInviteSlice>
    | ((s: GroupInviteSlice) => Partial<GroupInviteSlice>),
) => void;
type SliceGet = () => GroupInviteSlice;

export function errorOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

// 幂等合并：返回新列表或 null（无变化）。
// - 未见过的 id：追加；
// - 既有条目 pending：任何来料覆盖（pending 内容变化也生效）；
// - 既有条目已终态：同态重复幂等忽略，来料 pending 视为迟到事件忽略，
//   来料另一终态同样忽略（首个终态为准，防事件乱序回退）。
export function upsertGroupInviteInto(
  list: GroupInviteJson[],
  invite: GroupInviteJson,
): GroupInviteJson[] | null {
  const index = list.findIndex((i) => i.id === invite.id);
  if (index < 0) return [...list, invite];
  const existing = list[index]!;
  if (existing.state !== "pending") return null;
  if (
    existing.state === invite.state &&
    existing.delivered === invite.delivered &&
    existing.note === invite.note &&
    existing.groupName === invite.groupName &&
    existing.tsMs === invite.tsMs
  ) {
    return null;
  }
  const next = [...list];
  next[index] = invite;
  return next;
}

// 本端操作成功后的乐观收敛：pending → 目标终态（事件随后确认，幂等）。
function markLocal(get: SliceGet, inviteId: string, state: GroupInviteState): void {
  const invite = get().groupInvites.find((i) => i.id === inviteId);
  if (invite && invite.state === "pending") {
    get().upsertGroupInvite({ ...invite, state });
  }
}

export function createGroupInviteSlice(set: SliceSet, get: SliceGet): GroupInviteSlice {
  return {
    groupInvites: [],
    groupInvitesLoaded: false,
    groupInvitesError: null,

    loadGroupInvites: async () => {
      try {
        const invites = await ipc.chatGroupInvitesList();
        set({
          groupInvites: invites,
          groupInvitesLoaded: true,
          groupInvitesError: null,
        });
      } catch (error) {
        console.warn("[chat] 群邀请列表加载失败", error);
        set({ groupInvitesLoaded: true, groupInvitesError: errorOf(error) });
      }
    },

    upsertGroupInvite: (invite) => {
      set((s) => {
        const next = upsertGroupInviteInto(s.groupInvites, invite);
        return next ? { groupInvites: next } : {};
      });
    },

    sendGroupInvite: async (groupId, peerId, note) => {
      const invite = await ipc.chatGroupInviteSend(groupId, peerId, note);
      get().upsertGroupInvite(invite);
      return invite;
    },

    acceptGroupInvite: async (inviteId) => {
      await ipc.chatGroupInviteAccept(inviteId);
      markLocal(get, inviteId, "accepted");
    },

    rejectGroupInvite: async (inviteId, reason) => {
      await ipc.chatGroupInviteReject(inviteId, reason ?? null);
      markLocal(get, inviteId, "rejected");
    },
  };
}

// 铃铛徽标计数 selector（IMC3）：入群邀请与好友邀请两类 in 向待处理之和。
// 返回原始 number，规避 useSyncExternalStore 快照引用漂移（red-line 先例）。
export function selectPendingInviteBadgeCount(s: {
  groupInvites: GroupInviteJson[];
  invites: { direction: string }[];
}): number {
  const groupIn = s.groupInvites.filter(
    (i) => i.direction === "in" && i.state === "pending",
  ).length;
  const friendIn = s.invites.filter((i) => i.direction === "in").length;
  return groupIn + friendIn;
}
