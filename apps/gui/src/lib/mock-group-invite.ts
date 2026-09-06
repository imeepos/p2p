import type { GroupInviteJson, NodeEventJson } from "./ipc-types";
import { isMockFriend, activeMockChatRuntime, uuid } from "./mock-chat";
import {
  emitRoster,
  requireGroup,
  snapshotGroup,
  touchRoster,
  groupState,
} from "./mock-group-state";

// IMC3 入群邀请命令面 mock（冻结契约：命令名由 ipc.ts invoke 层承载，本文件
// 只提供 IpcBackend 群邀请段实现）。与 IMC1 后端同语义：邀请即达（mock 对端
// 恒在线）、同意后本地 roster 收编（emit chat_group_state 驱动 GUI 跳转时序）。
// 1:1 卡片消息（kind=groupInvite）经 mock chat runtime 落历史并发 chat_message。

const SELF_FALLBACK_NICKNAME = "mock-self";

export interface MockGroupInviteDeps {
  emit(event: NodeEventJson): void;
  selfPeerId(): string;
  selfNickname(): string;
}

interface MockGroupInviteState {
  invites: GroupInviteJson[];
}

const state: MockGroupInviteState = { invites: [] };

function findGroupOrCreateShadow(groupId: string, groupName: string, self: string) {
  const existing = groupState.groups.get(groupId);
  if (existing) return existing;
  // 影子群：仅承载邀请演示的名单容器，不进正式 mock 群列表语义
  const shadow = {
    groupId,
    name: groupName,
    owner: self,
    members: [self],
    rev: 1,
    state: "active" as const,
    tsMs: Date.now(),
  };
  groupState.groups.set(groupId, shadow);
  return shadow;
}

function cardBody(invite: GroupInviteJson, inviterNickname: string) {
  return {
    groupId: invite.groupId,
    groupName: invite.groupName,
    inviterNickname,
    note: invite.note,
  };
}

// 1:1 卡片消息落历史 + chat_message 事件（对方会话与本端会话各一份由调用方区分）
function appendCardMessage(invite: GroupInviteJson, sender: "me" | "them", inviterNickname: string) {
  const runtime = activeMockChatRuntime();
  const message = {
    id: uuid(),
    peer: sender === "me" ? invite.invitee : invite.inviter,
    sender,
    kind: "groupInvite" as const,
    tsMs: invite.tsMs,
    text: null,
    media: null,
    status: "delivered" as const,
    replyTo: null,
    groupInvite: cardBody(invite, inviterNickname),
  };
  runtime.appendMessage(message);
  runtime.emit({ type: "chat_message", peer: message.peer, message });
}

export function createMockGroupInviteBackend(deps: MockGroupInviteDeps) {
  function inviteError(inviteId: string): Error {
    return new Error("入群邀请不存在或已处理：" + inviteId);
  }

  return {
    async chatGroupInvitesList(): Promise<GroupInviteJson[]> {
      return state.invites.map((i) => ({ ...i }));
    },

    // 邀请好友入群：mock 对端恒在线，delivered 即真；同步落本端 out 卡片消息。
    async chatGroupInviteSend(
      groupId: string,
      peerId: string,
      note: string | null,
    ): Promise<GroupInviteJson> {
      const self = deps.selfPeerId();
      if (peerId === self) throw new Error("不能邀请自己入群");
      if (!isMockFriend(peerId)) {
        throw new Error("对方还不是好友，先加好友再拉群：" + peerId);
      }
      const group = groupState.groups.get(groupId);
      if (!group) throw new Error("群不存在：" + groupId);
      if (group.members.includes(peerId)) {
        throw new Error("该好友已在群内：" + peerId);
      }
      if (state.invites.some((i) => i.groupId === groupId && i.invitee === peerId && i.state === "pending")) {
        throw new Error("该好友已有待处理邀请：" + peerId);
      }
      const invite: GroupInviteJson = {
        id: uuid(),
        groupId,
        groupName: group.name,
        owner: group.owner,
        inviter: self,
        invitee: peerId,
        note: note ?? null,
        direction: "out",
        state: "pending",
        tsMs: Date.now(),
        delivered: true,
      };
      state.invites.push(invite);
      deps.emit({ type: "chat_group_invite", invite: { ...invite } });
      appendCardMessage(invite, "me", deps.selfNickname() || SELF_FALLBACK_NICKNAME);
      return { ...invite };
    },

    // 同意入群：pending → accepted；名单收编并回执 chat_group_state（跳转时序）。
    async chatGroupInviteAccept(inviteId: string): Promise<void> {
      const invite = state.invites.find((i) => i.id === inviteId);
      if (!invite || invite.state !== "pending" || invite.direction !== "in") {
        throw inviteError(inviteId);
      }
      invite.state = "accepted";
      const group = requireGroup(invite.groupId);
      if (!group.members.includes(deps.selfPeerId())) {
        group.members.push(deps.selfPeerId());
      }
      touchRoster(group);
      deps.emit({ type: "chat_group_invite", invite: { ...invite } });
      emitRoster(deps.emit, snapshotGroup(group));
    },

    // 拒绝入群：pending → rejected；reason 记录在案（mock 只保留终态）。
    async chatGroupInviteReject(inviteId: string, reason: string | null): Promise<void> {
      void reason;
      const invite = state.invites.find((i) => i.id === inviteId);
      if (!invite || invite.state !== "pending" || invite.direction !== "in") {
        throw inviteError(inviteId);
      }
      invite.state = "rejected";
      deps.emit({ type: "chat_group_invite", invite: { ...invite } });
    },
  };
}

// dev 注入入口：从指定好友视角造一条 in 向待处理邀请（演示消息中心与卡片）。
export function injectMockGroupInviteIncoming(
  deps: MockGroupInviteDeps,
  groupId: string,
  peerId: string,
  note: string | null,
): GroupInviteJson {
  const group = findGroupOrCreateShadow(groupId, "演示群-" + groupId.slice(0, 4), deps.selfPeerId());
  const invite: GroupInviteJson = {
    id: uuid(),
    groupId: group.groupId,
    groupName: group.name,
    owner: group.owner,
    inviter: peerId,
    invitee: deps.selfPeerId(),
    note: note ?? null,
    direction: "in",
    state: "pending",
    tsMs: Date.now(),
    delivered: true,
  };
  state.invites.push(invite);
  deps.emit({ type: "chat_group_invite", invite: { ...invite } });
  appendCardMessage(invite, "them", "mock-inviter");
  return { ...invite };
}
