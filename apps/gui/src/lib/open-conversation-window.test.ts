import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ConversationEntry } from "@/lib/conversation-entry";

const { toastError } = vi.hoisted(() => ({ toastError: vi.fn() }));
vi.mock("@/components/feedback/toast", () => ({ toastError }));

type OnceHandler = (event: { payload?: unknown }) => void;

const { created, getByLabel, fake } = vi.hoisted(() => ({
  created: [] as { label: string; options: Record<string, unknown> }[],
  getByLabel: vi.fn(),
  fake: { failCreate: false },
}));

vi.mock("@tauri-apps/api/webviewWindow", () => {
  class FakeWebviewWindow {
    handlers: Record<string, OnceHandler> = {};
    constructor(label: string, options: Record<string, unknown>) {
      created.push({ label, options });
      queueMicrotask(() => {
        const event = fake.failCreate ? "tauri://error" : "tauri://created";
        this.handlers[event]?.({ payload: "denied" });
      });
    }
    once(event: string, handler: OnceHandler) {
      this.handlers[event] = handler;
    }
    async setFocus() {}
    static getByLabel = getByLabel;
  }
  return { WebviewWindow: FakeWebviewWindow };
});

import { conversationUrl, openConversationWindow } from "@/lib/open-conversation-window";

function entry(overrides?: Partial<ConversationEntry>): ConversationEntry {
  return {
    id: "alice",
    kind: "friend",
    title: "Alice",
    subtitle: null,
    kindMark: { initial: "A", botIcon: false, groupBadge: false, dot: null },
    statusBadge: null,
    lastPreview: null,
    lastTsMs: 0,
    unread: 0,
    sendState: null,
    host: null,
    joinSeq: 0,
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  created.length = 0;
  getByLabel.mockResolvedValue(null);
  fake.failCreate = false;
  delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  vi.spyOn(window, "open").mockImplementation(() => null);
  vi.spyOn(console, "error").mockImplementation(() => {});
});

describe("open-conversation-window：独立窗口显示", () => {
  it("深链 URL 按会话种类拼键并编码 id", () => {
    expect(conversationUrl(entry({ id: "a b" }))).toBe("/chat?peer=a%20b");
    expect(conversationUrl(entry({ kind: "group", id: "g1" }))).toBe("/chat?group=g1");
    expect(conversationUrl(entry({ kind: "agent", id: "e1" }))).toBe("/chat?agent=e1");
  });

  it("浏览器环境回退 window.open", async () => {
    await openConversationWindow(entry());
    expect(window.open).toHaveBeenCalledWith("/chat?peer=alice", "_blank", "width=960,height=680");
    expect(toastError).not.toHaveBeenCalled();
  });

  it("Tauri 环境：创建新 WebviewWindow，标签含 kind 与清洗后的 id", async () => {
    (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    await openConversationWindow(entry({ id: "a/b:c" }));
    expect(created).toHaveLength(1);
    expect(created[0]?.label).toBe("chat-friend-abc");
    expect(created[0]?.options.url).toBe("/chat?peer=a%2Fb%3Ac");
    expect(toastError).not.toHaveBeenCalled();
  });

  it("同标签窗口已存在时聚焦复用，不再创建", async () => {
    (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    getByLabel.mockResolvedValue({ setFocus: vi.fn(async () => {}) });
    await openConversationWindow(entry());
    expect(created).toHaveLength(0);
    expect(toastError).not.toHaveBeenCalled();
  });

  it("创建失败走 toast + console.error，不静默", async () => {
    (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    fake.failCreate = true;
    await openConversationWindow(entry());
    expect(toastError).toHaveBeenCalledTimes(1);
    expect(vi.mocked(console.error)).toHaveBeenCalled();
  });
});
