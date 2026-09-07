import { create } from "zustand";

// 会话本地偏好（会话列表右键菜单的产物）：置顶/免打扰/标为未读/删除不显示。
// 与 ui-prefs-store 同型：localStorage 存档，损坏显式告警回默认，不静默。
const STORAGE_KEY = "p2p-gui.conversation-prefs";

/** 会话键：三来源（friend/group/agent）id 空间独立，拼 kind 防跨类碰撞 */
export type ConversationKey = string;

export function conversationKey(kind: string, id: string): ConversationKey {
  return kind + ":" + id;
}

export interface ConversationFlags {
  pinned: boolean;
  muted: boolean;
  /** 标为未读旗标：列表 unread 抬到 1，打开会话清除 */
  manualUnread: boolean;
}

const NO_FLAGS: ConversationFlags = { pinned: false, muted: false, manualUnread: false };

interface StoredPrefs {
  flags?: Record<string, Partial<ConversationFlags>>;
  dismissedAt?: Record<string, number>;
}

export interface ConversationPrefsState {
  flags: Record<string, ConversationFlags>;
  /** 删除/不显示时刻：晚于该时刻的新消息让会话回归列表（微信同款） */
  dismissedAt: Record<string, number>;
  togglePinned: (key: ConversationKey) => void;
  toggleMuted: (key: ConversationKey) => void;
  setManualUnread: (key: ConversationKey, value: boolean) => void;
  /** 不显示：仅从列表隐藏，新消息回归 */
  hideConversation: (key: ConversationKey, atMs: number) => void;
  /** 删除会话：隐藏并清空该会话旗标（回归时按全新会话呈现） */
  removeConversation: (key: ConversationKey, atMs: number) => void;
}

function loadStoredPrefs(): StoredPrefs {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as StoredPrefs;
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      throw new Error("conversation prefs 根节点不是对象");
    }
    return parsed;
  } catch (error) {
    console.warn("[conversation-prefs] localStorage 不可读，回退默认", error);
    return {};
  }
}

function sanitizeFlags(raw: StoredPrefs["flags"]): Record<string, ConversationFlags> {
  const flags: Record<string, ConversationFlags> = {};
  if (typeof raw !== "object" || raw === null) return flags;
  for (const [key, value] of Object.entries(raw)) {
    if (typeof value !== "object" || value === null) continue;
    flags[key] = {
      pinned: value.pinned === true,
      muted: value.muted === true,
      manualUnread: value.manualUnread === true,
    };
  }
  return flags;
}

function sanitizeDismissedAt(raw: StoredPrefs["dismissedAt"]): Record<string, number> {
  const dismissed: Record<string, number> = {};
  if (typeof raw !== "object" || raw === null) return dismissed;
  for (const [key, value] of Object.entries(raw)) {
    if (typeof value === "number" && Number.isFinite(value) && value > 0) {
      dismissed[key] = value;
    }
  }
  return dismissed;
}

function persist(state: {
  flags: Record<string, ConversationFlags>;
  dismissedAt: Record<string, number>;
}): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch (error) {
    console.warn("[conversation-prefs] localStorage 不可写，仅本次会话生效", error);
  }
}

export const useConversationPrefsStore = create<ConversationPrefsState>()((set, get) => {
  const stored = loadStoredPrefs();
  const initial = {
    flags: sanitizeFlags(stored.flags),
    dismissedAt: sanitizeDismissedAt(stored.dismissedAt),
  };
  const withPersist = (patch: Partial<ConversationPrefsState>) => {
    set(patch);
    persist(get());
  };
  return {
    ...initial,
    togglePinned: (key) => {
      const current = get().flags[key] ?? NO_FLAGS;
      withPersist({ flags: { ...get().flags, [key]: { ...current, pinned: !current.pinned } } });
    },
    toggleMuted: (key) => {
      const current = get().flags[key] ?? NO_FLAGS;
      withPersist({ flags: { ...get().flags, [key]: { ...current, muted: !current.muted } } });
    },
    setManualUnread: (key, value) => {
      const current = get().flags[key] ?? NO_FLAGS;
      withPersist({ flags: { ...get().flags, [key]: { ...current, manualUnread: value } } });
    },
    hideConversation: (key, atMs) => {
      withPersist({ dismissedAt: { ...get().dismissedAt, [key]: atMs } });
    },
    removeConversation: (key, atMs) => {
      const flags = { ...get().flags };
      delete flags[key];
      withPersist({ flags, dismissedAt: { ...get().dismissedAt, [key]: atMs } });
    },
  };
});
