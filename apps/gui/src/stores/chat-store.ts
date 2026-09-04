import { create } from "zustand";

import { ipc } from "@/lib/ipc";
import {
  mergeMessages,
  placeholderMessage,
  pushPending,
  retractPending,
  swapPending,
} from "@/lib/chat-local";
import type {
  ChatFriendJson,
  FriendInviteJson,
  ChatKind,
  ChatMediaInput,
  ChatMessageJson,
  ChatSendReport,
} from "@/lib/ipc-types";

const HISTORY_SIZE = 50;
let subscriptionStarted = false;

function errorOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export interface ChatStoreState {
  invites: FriendInviteJson[];
  friends: ChatFriendJson[];
  friendsLoaded: boolean;
  friendsError: string | null;
  selectedPeer: string | null;
  messagesByPeer: Record<string, ChatMessageJson[]>;
  lastMessageByPeer: Record<string, ChatMessageJson | null>;
  historyLoading: Record<string, boolean>;
  historyLoaded: Record<string, boolean>;
  hasMore: Record<string, boolean>;
  // 历史加载失败信号（IM-T50）：selectPeer/loadOlder 的 catch 落这里，成功清除
  historyError: Record<string, string | null>;
  olderError: Record<string, string | null>;
  loadInvites: () => Promise<void>;
  acceptInvite: (peer: string, nickname: string) => Promise<void>;
  rejectInvite: (peer: string) => Promise<void>;
  cancelInvite: (peer: string) => Promise<void>;
  loadFriends: () => Promise<void>;
  selectPeer: (peer: string) => Promise<void>;
  loadOlder: (peer: string) => Promise<void>;
  sendText: (
    peer: string,
    text: string,
    replyTo?: string | null,
  ) => Promise<ChatSendReport>;
  sendMedia: (
    peer: string,
    kind: ChatKind,
    media: ChatMediaInput,
    replyTo?: string | null,
  ) => Promise<ChatSendReport>;
  cancelPending: (peer: string, localMessageId: string) => void;
  forgetFriend: (peer: string) => void;
  /** chatFriendUpdate 成功后的本地收尾（IM-T43）；IPC 调用在 ChatFriendMoveDialog（调用点守卫要求） */
  updateFriendGroup: (peer: string, group: string | null) => void;
  subscribeEvents: () => Promise<void>;
}

export const useChatStore = create<ChatStoreState>()((set, get) => ({
  invites: [],
  friends: [],
  friendsLoaded: false,
  friendsError: null,
  selectedPeer: null,
  messagesByPeer: {},
  lastMessageByPeer: {},
  historyLoading: {},
  historyLoaded: {},
  hasMore: {},
  historyError: {},
  olderError: {},

  loadInvites: async () => {
    try {
      const invites = await ipc.chatInvitesList();
      set({ invites });
    } catch (error) {
      console.warn("[chat] 拉取邀请列表失败", error);
    }
  },

  acceptInvite: async (peer, nickname) => {
    await ipc.chatInviteAccept(peer, nickname);
    await Promise.all([get().loadFriends(), get().loadInvites()]);
  },

  rejectInvite: async (peer) => {
    await ipc.chatInviteReject(peer);
    await get().loadInvites();
  },

  cancelInvite: async (peer) => {
    await ipc.chatInviteCancel(peer);
    await get().loadInvites();
  },
  loadFriends: async () => {
    try {
      const friends = await ipc.chatFriendsList();
      const latest = await Promise.all(
        friends.map(async (friend, index) => {
          try {
            const page = await ipc.chatHistory(friend.peerId, null, 1);
            return { index, message: page[0] ?? null };
          } catch (error) {
            console.warn("[chat] 拉取好友摘要失败", friend.peerId, error);
            return { index, message: null };
          }
        }),
      );
      const messagesByPeer = { ...get().messagesByPeer };
      const lastMessageByPeer = { ...get().lastMessageByPeer };
      for (const item of latest) {
        const friend = friends[item.index];
        if (!friend || !item.message) continue;
        messagesByPeer[friend.peerId] = mergeMessages(
          messagesByPeer[friend.peerId] ?? [],
          [item.message],
        );
        lastMessageByPeer[friend.peerId] = item.message;
      }
      set({
        friends,
        friendsLoaded: true,
        friendsError: null,
        messagesByPeer,
        lastMessageByPeer,
      });
    } catch (error) {
      console.error("[chat] 好友列表加载失败", error);
      set({ friendsError: errorOf(error), friendsLoaded: true });
    }
  },

  selectPeer: async (peer) => {
    set({ selectedPeer: peer });
    if (get().historyLoaded[peer] || get().historyLoading[peer]) return;
    set((s) => ({
      historyLoading: { ...s.historyLoading, [peer]: true },
    }));
    try {
      const page = await ipc.chatHistory(peer, null, HISTORY_SIZE);
      set((s) => {
        const merged = mergeMessages(s.messagesByPeer[peer] ?? [], page);
        return {
          messagesByPeer: { ...s.messagesByPeer, [peer]: merged },
          lastMessageByPeer: {
            ...s.lastMessageByPeer,
            [peer]: merged[merged.length - 1] ?? null,
          },
          historyLoaded: { ...s.historyLoaded, [peer]: true },
          hasMore: { ...s.hasMore, [peer]: page.length === HISTORY_SIZE },
          historyError: { ...s.historyError, [peer]: null },
        };
      });
    } catch (error) {
      console.error("[chat] 历史加载失败", peer, error);
      set((s) => ({ historyError: { ...s.historyError, [peer]: errorOf(error) } }));
      throw error;
    } finally {
      set((s) => ({
        historyLoading: { ...s.historyLoading, [peer]: false },
      }));
    }
  },

  loadOlder: async (peer) => {
    const list = get().messagesByPeer[peer] ?? [];
    if (list.length === 0 || get().historyLoading[peer]) return;
    if (!get().hasMore[peer]) return;
    set((s) => ({
      historyLoading: { ...s.historyLoading, [peer]: true },
    }));
    try {
      const page = await ipc.chatHistory(peer, list[0]!.id, HISTORY_SIZE);
      set((s) => ({
        messagesByPeer: {
          ...s.messagesByPeer,
          [peer]: mergeMessages(s.messagesByPeer[peer] ?? [], page),
        },
        hasMore: { ...s.hasMore, [peer]: page.length === HISTORY_SIZE },
        olderError: { ...s.olderError, [peer]: null },
      }));
    } catch (error) {
      console.error("[chat] 加载更早历史失败", peer, error);
      set((s) => ({ olderError: { ...s.olderError, [peer]: errorOf(error) } }));
      throw error;
    } finally {
      set((s) => ({
        historyLoading: { ...s.historyLoading, [peer]: false },
      }));
    }
  },

  // 乐观发送：先落占位（pending），chatSend 返回后按占位 id 替换；失败移除占位并抛错。
  // replyTo 仅在有引用时出现在 IPC 调用上（无引用保持原有调用形状）。
  // 返回完整 ChatSendReport（IM-T51）：delivered=false 且 status=failed 的
  // mark_failed 路径不抛错，调用方靠 report 上浮失败信号。
  sendText: async (peer, text, replyTo) => {
    const trimmed = text.trim();
    if (!trimmed) throw new Error("chat text 为空");
    if (trimmed.length > 2000) throw new Error("chat text 超过 2000 字符");
    const placeholder = placeholderMessage(peer, "text", text, undefined, replyTo);
    pushPending(set, peer, placeholder);
    try {
      const report = replyTo
        ? await ipc.chatSend(peer, "text", trimmed, undefined, replyTo)
        : await ipc.chatSend(peer, "text", trimmed);
      swapPending(get, set, peer, placeholder.id, report.message);
      return report;
    } catch (error) {
      retractPending(set, peer, placeholder.id);
      console.error("[chat] 文本发送失败", error);
      throw error;
    }
  },

  sendMedia: async (peer, kind, media, replyTo) => {
    const placeholder = placeholderMessage(peer, kind, null, media, replyTo);
    pushPending(set, peer, placeholder);
    try {
      const report = replyTo
        ? await ipc.chatSend(peer, kind, undefined, media, replyTo)
        : await ipc.chatSend(peer, kind, undefined, media);
      swapPending(get, set, peer, placeholder.id, report.message);
      return report;
    } catch (error) {
      retractPending(set, peer, placeholder.id);
      console.error("[chat] 媒体发送失败", error);
      throw error;
    }
  },

  // 取消未发送附件：占位移除并回滚摘要；已替换（发送完成）则幂等无操作。
  cancelPending: (peer, localMessageId) => {
    retractPending(set, peer, localMessageId);
  },

  // chatFriendRemove 成功后的本地收尾：列表即时更新；被移除者是当前会话则清空选中
  // 回空态。IPC 调用在 ChatFriendRemoveDialog（界面入口层，调用点守卫要求）。
  // 不删本地消息历史（契约 §12.1），缓存保留供回加后继续使用。
  forgetFriend: (peer) => {
    set((s) => ({
      friends: s.friends.filter((f) => f.peerId !== peer),
      selectedPeer: s.selectedPeer === peer ? null : s.selectedPeer,
    }));
  },

  // 移动/移出分组成功后的本地收尾：列表按新分组即时渲染（未分组存 null 不落空串）。
  updateFriendGroup: (peer, group) => {
    set((s) => ({
      friends: s.friends.map((f) => (f.peerId === peer ? { ...f, group } : f)),
    }));
  },

  subscribeEvents: async () => {
    if (subscriptionStarted) return;
    subscriptionStarted = true;
    const unlisten = await ipc.onNodeEvent((event) => {
      if (event.type === "chat_message") {
        set((s) => {
          const peer = event.message.peer;
          const list = s.messagesByPeer[peer] ?? [];
          if (list.some((m) => m.id === event.message.id)) return s;
          return {
            messagesByPeer: {
              ...s.messagesByPeer,
              [peer]: mergeMessages(list, [event.message]),
            },
            lastMessageByPeer: {
              ...s.lastMessageByPeer,
              [peer]: event.message,
            },
          };
        });
      } else if (event.type === "chat_invite") {
        // 邀请生命周期：刷新邀请簿与好友簿（accepted 双向建簿）
        void get().loadInvites();
        void get().loadFriends();
      } else if (event.type === "chat_status") {
        set((s) => {
          const list = s.messagesByPeer[event.peer] ?? [];
          const next = list.map((m) =>
            m.id === event.messageId ? { ...m, status: event.status } : m,
          );
          if (next.every((m, index) => m === list[index])) return s;
          return {
            messagesByPeer: { ...s.messagesByPeer, [event.peer]: next },
          };
        });
      }
    });
    void unlisten;
  },
}));
