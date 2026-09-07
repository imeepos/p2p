// 分享发送目标模型（纯逻辑层，禁止引入 React/i18n 依赖）：
// 好友与群聊统一成可勾选目标行；选中集合与发送结果归并为摘要。
import type { ChatFriendJson, GroupJson } from "@/lib/ipc-types";

export type ShareTargetKind = "friend" | "group";

export interface ShareTarget {
  /** 稳定键："friend:<peerId>" / "group:<groupId>"，跨好友群聊命名空间不撞 */
  key: string;
  kind: ShareTargetKind;
  /** peerId（好友）或 groupId（群聊） */
  id: string;
  label: string;
}

export function targetKey(kind: ShareTargetKind, id: string): string {
  return kind + ":" + id;
}

export function buildTargets(friends: ChatFriendJson[], groups: GroupJson[]): ShareTarget[] {
  const friendRows = friends.map((f) => ({
    key: targetKey("friend", f.peerId),
    kind: "friend" as const,
    id: f.peerId,
    label: f.nickname || f.peerId.slice(0, 8),
  }));
  const groupRows = groups
    .filter((g) => g.state === "active")
    .map((g) => ({
      key: targetKey("group", g.groupId),
      kind: "group" as const,
      id: g.groupId,
      label: g.name,
    }));
  return [...friendRows, ...groupRows];
}

/** 选中集合切换：已选即移除，未选即加入（无序，按 targets 顺序渲染） */
export function toggleSelected(selected: string[], key: string): string[] {
  return selected.includes(key) ? selected.filter((k) => k !== key) : [...selected, key];
}

export interface SendOutcome {
  key: string;
  ok: boolean;
}

/** 发送结果归并：全成功 / 部分失败计数（失败路径显式可观测，不静默吞） */
export function summarizeSend(outcomes: SendOutcome[]): { sent: number; failed: number } {
  return {
    sent: outcomes.filter((o) => o.ok).length,
    failed: outcomes.filter((o) => !o.ok).length,
  };
}
