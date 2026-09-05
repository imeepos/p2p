import type { GroupJson, IpcBackend } from "./ipc-types";
import { newGroupMessageId as uuid } from "./mock-group-state";
import { MAX_GROUP_MEMBERS, validateGroupChatName } from "./chat-limits";
import {
  emitRoster,
  groupState,
  requireGroup,
  requireOwner,
  snapshotGroup,
  touchRoster,
  type MockGroupChatDeps,
} from "./mock-group-state";

// 群 roster 操作面（im-group-design §5）：建群/列表/邀请/移除/退群/改名。
// owner 权威模型：rev 仅 owner 单调递增；退群/被踢置位不删数据；
// 每次名单变更发 chat_group_state 回执；owner-only 违例给可读中文 Err。

export type MockGroupRosterBackend = Pick<
  IpcBackend,
  | "groupCreate"
  | "groupList"
  | "groupInvite"
  | "groupKick"
  | "groupLeave"
  | "groupRename"
  | "groupDisband"
>;

// 建/邀成员公共校验：名单去重非空、不含本机、全部在好友簿、总量 ≤32。
function validateMemberList(
  deps: MockGroupChatDeps,
  self: string,
  memberIds: string[],
  existing: number,
  emptyHint: string,
): string[] {
  const unique = [...new Set(memberIds)];
  if (unique.length === 0) throw new Error(emptyHint);
  if (unique.includes(self)) throw new Error("成员名单不能包含本机");
  for (const member of unique) {
    if (!deps.isFriend(member)) throw new Error(`成员不在好友簿：${member}`);
  }
  if (existing + unique.length > MAX_GROUP_MEMBERS) {
    throw new Error(`群成员超过 ${MAX_GROUP_MEMBERS} 上限`);
  }
  return unique;
}

function requireActive(group: GroupJson): void {
  if (group.state !== "active") {
    throw new Error(`群当前不可用（${group.state}）`);
  }
}

export function createMockGroupRosterOps(
  deps: MockGroupChatDeps,
): MockGroupRosterBackend {
  const self = () => deps.selfPeerId();

  return {
    // 设计 §5 建群：校验通过后本地 rev=0 建群并推 roster。
    async groupCreate(name, memberIds) {
      const invalidName = validateGroupChatName(name);
      if (invalidName) throw new Error(invalidName);
      const unique = validateMemberList(
        deps,
        self(),
        memberIds,
        1,
        "建群至少需要一名其他成员",
      );
      const group: GroupJson = {
        groupId: uuid(),
        name: name.trim(),
        owner: self(),
        members: [self(), ...unique],
        rev: 0,
        state: "active",
        tsMs: Date.now(),
      };
      groupState.groups.set(group.groupId, group);
      emitRoster(deps.emit, group);
      return snapshotGroup(group);
    },

    // 全量含 left/kicked/disbanded（GUI 按 state 过滤/置底）。
    async groupList() {
      return [...groupState.groups.values()].map(snapshotGroup);
    },

    // owner-only；受邀者 ∈ 好友簿且不在群；群 <32；rev+1 推全体（含新成员）。
    async groupInvite(groupId, memberIds) {
      const group = requireGroup(groupId);
      requireOwner(group, self());
      requireActive(group);
      const unique = validateMemberList(
        deps,
        self(),
        memberIds,
        group.members.length,
        "邀请名单为空",
      );
      for (const member of unique) {
        if (group.members.includes(member)) {
          throw new Error(`已在群中：${member}`);
        }
      }
      group.members = [...group.members, ...unique];
      touchRoster(group);
      emitRoster(deps.emit, group);
      return snapshotGroup(group);
    },

    // owner-only；不能移除群主；rev+1 推余员（G_KICK 通知由真实实现补发）。
    async groupKick(groupId, memberId) {
      const group = requireGroup(groupId);
      requireOwner(group, self());
      if (memberId === group.owner) throw new Error("不能移除群主");
      if (!group.members.includes(memberId)) {
        throw new Error(`该成员不在群中：${memberId}`);
      }
      group.members = group.members.filter((m) => m !== memberId);
      touchRoster(group);
      emitRoster(deps.emit, group);
      return snapshotGroup(group);
    },

    // 本端 state=left（历史保留）；群主不能退群（退群即解散，v1 无解散命令面）。
    async groupLeave(groupId) {
      const group = requireGroup(groupId);
      const me = self();
      if (group.owner === me) throw new Error("群主不能退群");
      requireActive(group);
      if (!group.members.includes(me)) {
        throw new Error(`你已不在该群：${groupId}`);
      }
      group.state = "left";
      group.tsMs = Date.now();
      emitRoster(deps.emit, group);
      return snapshotGroup(group);
    },

    // owner-only；群名 trim 后 1..=64；rev+1 推 roster。
    async groupRename(groupId, name) {
      const group = requireGroup(groupId);
      requireOwner(group, self());
      const invalidName = validateGroupChatName(name);
      if (invalidName) throw new Error(invalidName);
      group.name = name.trim();
      touchRoster(group);
      emitRoster(deps.emit, group);
      return snapshotGroup(group);
    },

    // owner-only；仅 active 可解散（重复解散显式 Err）；rev+1，本端 state=disbanded
    //（成员端 G_KICK 通知由真实实现补发，mock 单机无对端）。
    async groupDisband(groupId) {
      const group = requireGroup(groupId);
      requireOwner(group, self());
      requireActive(group);
      group.state = "disbanded";
      touchRoster(group);
      emitRoster(deps.emit, group);
      return snapshotGroup(group);
    },
  };
}
