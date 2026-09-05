import { create } from "zustand";

import { ipc } from "@/lib/ipc";
import { MAX_TEXT_CHARS } from "@/lib/chat-limits";
import type {
  ChatFriendJson,
  ChatKind,
  ChatMediaInput,
  GroupJson,
  GroupMessageJson,
  GroupSendReport,
} from "@/lib/ipc-types";
import {
  groupMessagesAfterEvent,
  mergeGroupMessages,
  placeholderGroupMessage,
  pushGroupPending,
  retractGroupPending,
  swapGroupPending,
} from "./group-local";

const HISTORY_SIZE = 50;
let subscriptionStarted = false;

function errorOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export interface GroupStoreState {
  groups: GroupJson[];
  groupsLoaded: boolean;
  groupsError: string | null;
  friends: ChatFriendJson[];
  friendsLoaded: boolean;
  selfPeerId: string | null;
  selectedGroupId: string | null;
  messagesByGroup: Record<string, GroupMessageJson[]>;
  lastMessageByGroup: Record<string, GroupMessageJson | null>;
  historyLoading: Record<string, boolean>;
  historyLoaded: Record<string, boolean>;
  hasMore: Record<string, boolean>;
  historyError: Record<string, string | null>;
  olderError: Record<string, string | null>;
  refreshSelf: () => Promise<void>;
  loadGroups: () => Promise<void>;
  ensureFriends: () => Promise<void>;
  selectGroup: (groupId: string) => Promise<void>;
  loadOlder: (groupId: string) => Promise<void>;
  sendText: (groupId: string, text: string, replyTo?: string | null) => Promise<GroupSendReport>;
  sendMedia: (
    groupId: string,
    kind: ChatKind,
    media: ChatMediaInput,
    replyTo?: string | null,
  ) => Promise<GroupSendReport>;
  cancelPending: (groupId: string, localMessageId: string) => void;
  invite: (groupId: string, memberIds: string[]) => Promise<GroupJson>;
  kick: (groupId: string, memberId: string) => Promise<GroupJson>;
  leave: (groupId: string) => Promise<GroupJson>;
  rename: (groupId: string, name: string) => Promise<GroupJson>;
  disband: (groupId: string) => Promise<GroupJson>;
  upsertGroup: (group: GroupJson) => void;
  subscribeEvents: () => Promise<void>;
}

export const useGroupStore = create<GroupStoreState>()((set, get) => ({
  groups: [],
  groupsLoaded: false,
  groupsError: null,
  friends: [],
  friendsLoaded: false,
  selfPeerId: null,
  selectedGroupId: null,
  messagesByGroup: {},
  lastMessageByGroup: {},
  historyLoading: {},
  historyLoaded: {},
  hasMore: {},
  historyError: {},
  olderError: {},

  // 本机 PeerId（senderId===self 判定输入）：节点未运行时为 null（契约不暴露），
  // 视图在节点停止态禁用输入，不出现误判路径。
  refreshSelf: async () => {
    try {
      const status = await ipc.nodeStatus();
      set({ selfPeerId: status.peerId });
    } catch (error) {
      console.error("[group] 本机 PeerId 获取失败", error);
    }
  },

  loadGroups: async () => {
    try {
      const groups = await ipc.groupList();
      set({ groups, groupsLoaded: true, groupsError: null });
    } catch (error) {
      console.error("[group] 群列表加载失败", error);
      set({ groupsError: errorOf(error), groupsLoaded: true });
    }
  },

  // 群视图专用轻量好友簿（昵称解析/邀请勾选）；不拉 1:1 消息摘要。
  ensureFriends: async () => {
    if (get().friendsLoaded) return;
    try {
      const friends = await ipc.chatFriendsList();
      set({ friends, friendsLoaded: true });
    } catch (error) {
      console.error("[group] 好友簿加载失败", error);
      set({ friendsLoaded: true });
    }
  },

  selectGroup: async (groupId) => {
    set({ selectedGroupId: groupId });
    if (get().historyLoaded[groupId] || get().historyLoading[groupId]) return;
    set((s) => ({ historyLoading: { ...s.historyLoading, [groupId]: true } }));
    try {
      const page = await ipc.groupHistory(groupId, null, HISTORY_SIZE);
      set((s) => ({
        messagesByGroup: {
          ...s.messagesByGroup,
          [groupId]: mergeGroupMessages(s.messagesByGroup[groupId] ?? [], page),
        },
        historyLoaded: { ...s.historyLoaded, [groupId]: true },
        hasMore: { ...s.hasMore, [groupId]: page.length === HISTORY_SIZE },
        historyError: { ...s.historyError, [groupId]: null },
      }));
    } catch (error) {
      console.error("[group] 群历史加载失败", groupId, error);
      set((s) => ({ historyError: { ...s.historyError, [groupId]: errorOf(error) } }));
      throw error;
    } finally {
      set((s) => ({ historyLoading: { ...s.historyLoading, [groupId]: false } }));
    }
  },

  loadOlder: async (groupId) => {
    const list = get().messagesByGroup[groupId] ?? [];
    if (list.length === 0 || get().historyLoading[groupId]) return;
    if (!get().hasMore[groupId]) return;
    set((s) => ({ historyLoading: { ...s.historyLoading, [groupId]: true } }));
    try {
      const page = await ipc.groupHistory(groupId, list[0]!.id, HISTORY_SIZE);
      set((s) => ({
        messagesByGroup: {
          ...s.messagesByGroup,
          [groupId]: mergeGroupMessages(s.messagesByGroup[groupId] ?? [], page),
        },
        hasMore: { ...s.hasMore, [groupId]: page.length === HISTORY_SIZE },
        olderError: { ...s.olderError, [groupId]: null },
      }));
    } catch (error) {
      console.error("[group] 更早群历史加载失败", groupId, error);
      set((s) => ({ olderError: { ...s.olderError, [groupId]: errorOf(error) } }));
      throw error;
    } finally {
      set((s) => ({ historyLoading: { ...s.historyLoading, [groupId]: false } }));
    }
  },

  // 乐观发送（同 1:1 纪律）：占位 pending → groupSend 返回换真身；失败回滚并抛错。
  sendText: async (groupId, text, replyTo) => {
    const trimmed = text.trim();
    if (!trimmed) throw new Error("group text 为空");
    if (trimmed.length > MAX_TEXT_CHARS) throw new Error("group text 超过 2000 字符");
    const self = get().selfPeerId ?? "";
    const placeholder = placeholderGroupMessage(groupId, "text", text, self, undefined, replyTo);
    pushGroupPending(set, groupId, placeholder);
    try {
      const report = await ipc.groupSend(groupId, "text", trimmed, undefined, replyTo);
      swapGroupPending(get, set, groupId, placeholder.id, report.message);
      return report;
    } catch (error) {
      retractGroupPending(set, groupId, placeholder.id);
      console.error("[group] 文本发送失败", error);
      throw error;
    }
  },

  sendMedia: async (groupId, kind, media, replyTo) => {
    const self = get().selfPeerId ?? "";
    const placeholder = placeholderGroupMessage(groupId, kind, null, self, media, replyTo);
    pushGroupPending(set, groupId, placeholder);
    try {
      const report = await ipc.groupSend(groupId, kind, undefined, media, replyTo);
      swapGroupPending(get, set, groupId, placeholder.id, report.message);
      return report;
    } catch (error) {
      retractGroupPending(set, groupId, placeholder.id);
      console.error("[group] 附件发送失败", error);
      throw error;
    }
  },

  cancelPending: (groupId, localMessageId) => {
    retractGroupPending(set, groupId, localMessageId);
  },

  invite: async (groupId, memberIds) => {
    const group = await ipc.groupInvite(groupId, memberIds);
    get().upsertGroup(group);
    return group;
  },

  kick: async (groupId, memberId) => {
    const group = await ipc.groupKick(groupId, memberId);
    get().upsertGroup(group);
    return group;
  },

  leave: async (groupId) => {
    const group = await ipc.groupLeave(groupId);
    get().upsertGroup(group);
    return group;
  },

  rename: async (groupId, name) => {
    const group = await ipc.groupRename(groupId, name);
    get().upsertGroup(group);
    return group;
  },

  disband: async (groupId) => {
    const group = await ipc.groupDisband(groupId);
    get().upsertGroup(group);
    return group;
  },

  // 建群/roster 事件统一入口：按 groupId 替换或插入，保序其余不动。
  upsertGroup: (group) => {
    set((s) => ({
      groups: s.groups.some((g) => g.groupId === group.groupId)
        ? s.groups.map((g) => (g.groupId === group.groupId ? group : g))
        : [...s.groups, group],
    }));
  },

  subscribeEvents: async () => {
    if (subscriptionStarted) return;
    subscriptionStarted = true;
    const unlisten = await ipc.onNodeEvent((event) => {
      if (event.type === "chat_group_state") {
        get().upsertGroup(event.group);
        return;
      }
      const patch = groupMessagesAfterEvent(
        get().messagesByGroup,
        get().lastMessageByGroup,
        event,
      );
      if (patch) set(patch);
    });
    void unlisten;
  },
}));
