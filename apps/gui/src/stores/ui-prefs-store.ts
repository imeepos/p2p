import { create } from "zustand";

// GUI 本机 UI 偏好（localStorage 存档）：与 theme/i18n 同型——损坏显式告警
// 回默认值，不静默；新增偏好只加字段，读侧按字段判型。
const STORAGE_KEY = "p2p-gui.ui-prefs";

interface StoredUiPrefs {
  /** 已退出/已解散群聊是否显示在会话列表（缺省 false = 隐藏） */
  showInactiveGroups?: unknown;
}

export interface UiPrefsState {
  showInactiveGroups: boolean;
  setShowInactiveGroups: (value: boolean) => void;
}

function loadShowInactiveGroups(): boolean {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return false;
    const parsed = JSON.parse(raw) as StoredUiPrefs;
    return parsed.showInactiveGroups === true;
  } catch (error) {
    console.warn("[ui-prefs] localStorage 不可读，回退默认（隐藏已退群）", error);
    return false;
  }
}

function persistShowInactiveGroups(value: boolean): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ showInactiveGroups: value }));
  } catch (error) {
    console.warn("[ui-prefs] localStorage 不可写，开关仅本次会话生效", error);
  }
}

export const useUiPrefsStore = create<UiPrefsState>()((set) => ({
  showInactiveGroups: loadShowInactiveGroups(),
  setShowInactiveGroups: (value) => {
    persistShowInactiveGroups(value);
    set({ showInactiveGroups: value });
  },
}));
