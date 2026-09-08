import type { ConversationEntry } from "@/lib/conversation-entry";
import { sortEntries } from "@/lib/conversation-entry";
import { conversationKey } from "@/stores/conversation-prefs-store";
import type { ConversationFlags } from "@/stores/conversation-prefs-store";

// 右键菜单偏好在列表层的机械语义（纯函数，可独立单测）：
// - 删除/不显示：dismissedAt 起隐藏，晚于该时刻的新消息让会话回归；
// - 标为未读：unread 下限抬到 1，打开会话清旗；
// - 置顶：稳定分区到列表头，分区内保持既有排序。

export function isVisibleAfterDismiss(dismissedAtMs: number, lastTsMs: number): boolean {
  return lastTsMs > dismissedAtMs;
}

export function overlayUnread(unread: number, manualUnread: boolean): number {
  return manualUnread && unread < 1 ? 1 : unread;
}

export interface ConversationPrefsSnapshot {
  flags: Record<string, ConversationFlags>;
  dismissedAt: Record<string, number>;
}

export function applyConversationPrefs(
  entries: ConversationEntry[],
  prefs: ConversationPrefsSnapshot,
): ConversationEntry[] {
  const visible = entries.filter((entry) => {
    const dismissed = prefs.dismissedAt[conversationKey(entry.kind, entry.id)];
    return dismissed === undefined || isVisibleAfterDismiss(dismissed, entry.lastTsMs);
  });
  const withUnread = visible.map((entry) => {
    const flag = prefs.flags[conversationKey(entry.kind, entry.id)];
    if (!flag || !flag.manualUnread) return entry;
    return { ...entry, unread: overlayUnread(entry.unread, true) };
  });
  const pinned = withUnread.filter(
    (entry) => prefs.flags[conversationKey(entry.kind, entry.id)]?.pinned === true,
  );
  const rest = withUnread.filter(
    (entry) => prefs.flags[conversationKey(entry.kind, entry.id)]?.pinned !== true,
  );
  return [...sortEntries(pinned), ...sortEntries(rest)];
}
