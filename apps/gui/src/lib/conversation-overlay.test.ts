import { describe, expect, it } from "vitest";

import type { ConversationEntry } from "@/lib/conversation-entry";
import {
  applyConversationPrefs,
  isVisibleAfterDismiss,
  overlayUnread,
} from "@/lib/conversation-overlay";

function entry(id: string, overrides?: Partial<ConversationEntry>): ConversationEntry {
  return {
    id,
    kind: "friend",
    title: id,
    subtitle: null,
    kindMark: { initial: id[0] ?? "?", botIcon: false, groupBadge: false, dot: null },
    statusBadge: null,
    lastPreview: null,
    lastTsMs: 1000,
    unread: 0,
    sendState: null,
    host: null,
    joinSeq: 0,
    ...overrides,
  };
}

describe("conversation-overlay：右键菜单偏好的列表语义", () => {
  it("isVisibleAfterDismiss：晚于隐藏时点的消息让会话回归", () => {
    expect(isVisibleAfterDismiss(1000, 999)).toBe(false);
    expect(isVisibleAfterDismiss(1000, 1001)).toBe(true);
  });

  it("overlayUnread：标为未读把 0 抬到 1，不动真实未读", () => {
    expect(overlayUnread(0, true)).toBe(1);
    expect(overlayUnread(3, true)).toBe(3);
    expect(overlayUnread(0, false)).toBe(0);
  });

  it("dismissed 且无新消息的会话被隐藏", () => {
    const prefs = { flags: {}, dismissedAt: { "friend:a": 2000 } };
    expect(applyConversationPrefs([entry("a")], prefs)).toEqual([]);
  });

  it("dismissed 后 lastTsMs 推进则回归列表", () => {
    const prefs = { flags: {}, dismissedAt: { "friend:a": 2000 } };
    const visible = applyConversationPrefs([entry("a", { lastTsMs: 3000 })], prefs);
    expect(visible).toHaveLength(1);
  });

  it("标为未读旗标把 unread 抬到 1", () => {
    const prefs = { flags: { "friend:a": { pinned: false, muted: false, manualUnread: true } }, dismissedAt: {} };
    const [first] = applyConversationPrefs([entry("a")], prefs);
    expect(first?.unread).toBe(1);
  });

  it("置顶分区到列表头，分区内保持时间序", () => {
    const prefs = {
      flags: { "friend:old": { pinned: true, muted: false, manualUnread: false } },
      dismissedAt: {},
    };
    const visible = applyConversationPrefs(
      [entry("new", { lastTsMs: 3000 }), entry("old", { lastTsMs: 1000 }), entry("mid", { lastTsMs: 2000 })],
      prefs,
    );
    expect(visible.map((e) => e.id)).toEqual(["old", "new", "mid"]);
  });
});
