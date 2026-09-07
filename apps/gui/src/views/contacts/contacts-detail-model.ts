import type { AcpEndpoint } from "@/acp/protocol";
import { visibleGroups } from "@/lib/conversation-entry";
import type { ChatFriendJson, FriendInviteJson, GroupJson, InviteDirectionJson } from "@/lib/ipc-types";

// 资料卡选中模型（双栏改版）：选中项是四类实体之一；store 数据变化后
// 选中项失效（好友被删/退群/endpoint 移除/邀请处理完）即由调用方回退到
// 首个可用实体，资料卡不指向不存在的关系。纯函数，独立成文件便于单测。

export type ContactSelection =
  | { kind: "friend"; peerId: string }
  | { kind: "group"; groupId: string }
  | { kind: "agent"; endpointId: string }
  | { kind: "invite"; peerId: string; direction: InviteDirectionJson };

export interface ContactEntity {
  selection: ContactSelection;
  friend?: ChatFriendJson;
  group?: GroupJson;
  agent?: AcpEndpoint;
  invite?: FriendInviteJson;
}

export interface ContactsData {
  friends: ChatFriendJson[];
  invites: FriendInviteJson[];
  groups: GroupJson[];
  agents: AcpEndpoint[];
}

export function selectionKey(selection: ContactSelection): string {
  switch (selection.kind) {
    case "friend":
      return "friend:" + selection.peerId;
    case "group":
      return "group:" + selection.groupId;
    case "agent":
      return "agent:" + selection.endpointId;
    case "invite":
      return "invite:" + selection.direction + ":" + selection.peerId;
  }
}

function agentIdOf(endpoint: AcpEndpoint): string {
  return endpoint.endpointId ?? endpoint.wsUrl;
}

export function resolveEntity(
  selection: ContactSelection,
  data: ContactsData,
): ContactEntity | null {
  switch (selection.kind) {
    case "friend": {
      const friend = data.friends.find((f) => f.peerId === selection.peerId);
      return friend ? { selection, friend } : null;
    }
    case "group": {
      // 与通讯录行同口径：非 active 群不作为有效选中
      const group = visibleGroups(data.groups, false).find(
        (g) => g.groupId === selection.groupId,
      );
      return group ? { selection, group } : null;
    }
    case "agent": {
      const agent = data.agents.find((e) => agentIdOf(e) === selection.endpointId);
      return agent ? { selection, agent } : null;
    }
    case "invite": {
      const invite = data.invites.find(
        (i) => i.peerId === selection.peerId && i.direction === selection.direction,
      );
      return invite ? { selection, invite } : null;
    }
  }
}

// 回退顺序与左栏节顺序一致：好友 → 群 → Agent → 邀请（in 优先于 out）。
export function firstAvailableEntity(data: ContactsData): ContactEntity | null {
  const friend = data.friends[0];
  if (friend) return { selection: { kind: "friend", peerId: friend.peerId }, friend };
  const group = visibleGroups(data.groups, false)[0];
  if (group) return { selection: { kind: "group", groupId: group.groupId }, group };
  const agent = data.agents[0];
  if (agent) return { selection: { kind: "agent", endpointId: agentIdOf(agent) }, agent };
  const invites = [...data.invites].sort(
    (a, b) => (a.direction === "in" ? 0 : 1) - (b.direction === "in" ? 0 : 1),
  );
  const invite = invites[0];
  if (invite) {
    return {
      selection: { kind: "invite", peerId: invite.peerId, direction: invite.direction },
      invite,
    };
  }
  return null;
}
