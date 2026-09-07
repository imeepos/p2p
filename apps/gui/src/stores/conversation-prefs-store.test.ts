import { beforeEach, describe, expect, it, vi } from "vitest";

// 每用例重载模块：模块在 import 时读 localStorage，需重求值才能验证装载路径。
async function freshStore() {
  return await import("@/stores/conversation-prefs-store");
}

const KEY = "p2p-gui.conversation-prefs";

beforeEach(() => {
  vi.resetModules();
  localStorage.clear();
});

describe("conversation-prefs：会话本地偏好", () => {
  it("空存档全默认；toggle 翻转并写穿 localStorage", async () => {
    const { useConversationPrefsStore } = await freshStore();
    expect(useConversationPrefsStore.getState().flags).toEqual({});
    useConversationPrefsStore.getState().togglePinned("friend:alice");
    useConversationPrefsStore.getState().toggleMuted("friend:alice");
    const flags = useConversationPrefsStore.getState().flags["friend:alice"];
    expect(flags).toEqual({ pinned: true, muted: true, manualUnread: false });
    expect(JSON.parse(localStorage.getItem(KEY) ?? "{}").flags["friend:alice"].pinned).toBe(true);
  });

  it("setManualUnread 两个方向都可写", async () => {
    const { useConversationPrefsStore } = await freshStore();
    useConversationPrefsStore.getState().setManualUnread("group:g1", true);
    expect(useConversationPrefsStore.getState().flags["group:g1"].manualUnread).toBe(true);
    useConversationPrefsStore.getState().setManualUnread("group:g1", false);
    expect(useConversationPrefsStore.getState().flags["group:g1"].manualUnread).toBe(false);
  });

  it("hide 仅记 dismissedAt；remove 记 dismissedAt 且清旗标", async () => {
    const { useConversationPrefsStore } = await freshStore();
    useConversationPrefsStore.getState().togglePinned("friend:alice");
    useConversationPrefsStore.getState().hideConversation("friend:alice", 1000);
    expect(useConversationPrefsStore.getState().dismissedAt["friend:alice"]).toBe(1000);
    expect(useConversationPrefsStore.getState().flags["friend:alice"].pinned).toBe(true);

    useConversationPrefsStore.getState().togglePinned("friend:bob");
    useConversationPrefsStore.getState().removeConversation("friend:bob", 2000);
    expect(useConversationPrefsStore.getState().dismissedAt["friend:bob"]).toBe(2000);
    expect(useConversationPrefsStore.getState().flags["friend:bob"]).toBeUndefined();
  });

  it("重装载读到已存档偏好", async () => {
    const { useConversationPrefsStore } = await freshStore();
    useConversationPrefsStore.getState().toggleMuted("agent:e1");
    useConversationPrefsStore.getState().hideConversation("agent:e1", 3000);
    const reloaded = await freshStore();
    const state = reloaded.useConversationPrefsStore.getState();
    expect(state.flags["agent:e1"].muted).toBe(true);
    expect(state.dismissedAt["agent:e1"]).toBe(3000);
  });

  it("损坏 JSON 显式告警并回默认", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    localStorage.setItem(KEY, "{not-json");
    const { useConversationPrefsStore } = await freshStore();
    expect(useConversationPrefsStore.getState().flags).toEqual({});
    expect(useConversationPrefsStore.getState().dismissedAt).toEqual({});
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });

  it("非法字段值被逐字段清洗（严格布尔/正数时刻）", async () => {
    vi.resetModules();
    localStorage.setItem(
      KEY,
      JSON.stringify({
        flags: { "friend:x": { pinned: "yes", muted: 1, manualUnread: true } },
        dismissedAt: { "friend:y": "soon" },
      }),
    );
    const cleaned = await freshStore();
    const state = cleaned.useConversationPrefsStore.getState();
    expect(state.flags["friend:x"]).toEqual({ pinned: false, muted: false, manualUnread: true });
    expect(state.dismissedAt["friend:y"]).toBeUndefined();
  });
});
