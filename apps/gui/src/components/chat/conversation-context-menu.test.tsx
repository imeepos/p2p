import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import "@/i18n";
import { useChatStore } from "@/stores/chat-store";
import {
  conversationKey,
  useConversationPrefsStore,
} from "@/stores/conversation-prefs-store";
import type { ConversationEntry } from "@/lib/conversation-entry";
import { ConversationContextMenu } from "./conversation-context-menu";

function entry(overrides?: Partial<ConversationEntry>): ConversationEntry {
  return {
    id: "alice",
    kind: "friend",
    title: "Alice",
    subtitle: null,
    kindMark: { initial: "A", botIcon: false, groupBadge: false, dot: null },
    statusBadge: null,
    lastPreview: "hi",
    lastTsMs: 1000,
    unread: 0,
    sendState: null,
    host: null,
    joinSeq: 0,
    ...overrides,
  };
}

const ANCHOR = { x: 40, y: 60 };

function renderMenu(node: React.ReactElement) {
  return render(node);
}
void renderMenu;

beforeEach(() => {
  localStorage.clear();
  useConversationPrefsStore.setState({ flags: {}, dismissedAt: {} });
  useChatStore.setState({ unreadByPeer: {} });
});

describe("ConversationContextMenu（会话右键菜单）", () => {
  it("无 entry/anchor 时不出渲染任何菜单", () => {
    const { container } = render(<ConversationContextMenu entry={null} anchor={null} onClose={() => {}} />);
    expect(container).toBeEmptyDOMElement();
    expect(document.querySelector("[data-testid='conversation-context-menu']")).toBeNull();
  });

  it("打开渲染六项：置顶/标为未读/消息免打扰/独立窗口/不显示/删除", () => {
    render(<ConversationContextMenu entry={entry()} anchor={ANCHOR} onClose={() => {}} />);
    for (const testId of [
      "conversation-menu-pin",
      "conversation-menu-unread",
      "conversation-menu-mute",
      "conversation-menu-window",
      "conversation-menu-hide",
      "conversation-menu-delete",
    ]) {
      expect(screen.getByTestId(testId)).toBeTruthy();
    }
    expect(screen.getByTestId("conversation-menu-pin").textContent).toContain("置顶");
    expect(screen.getByTestId("conversation-menu-unread").textContent).toContain("标为未读");
  });

  it("置顶写入偏好且文案翻转为取消置顶", () => {
    const { rerender } = render(
      <ConversationContextMenu entry={entry()} anchor={ANCHOR} onClose={() => {}} />,
    );
    fireEvent.click(screen.getByTestId("conversation-menu-pin"));
    const key = conversationKey("friend", "alice");
    expect(useConversationPrefsStore.getState().flags[key]?.pinned).toBe(true);
    rerender(<ConversationContextMenu entry={entry()} anchor={ANCHOR} onClose={() => {}} />);
    expect(screen.getByTestId("conversation-menu-pin").textContent).toContain("取消置顶");
  });

  it("标为未读置旗标；再点标为已读清旗标并清 store 未读", () => {
    const { rerender } = render(
      <ConversationContextMenu entry={entry()} anchor={ANCHOR} onClose={() => {}} />,
    );
    fireEvent.click(screen.getByTestId("conversation-menu-unread"));
    const key = conversationKey("friend", "alice");
    expect(useConversationPrefsStore.getState().flags[key]?.manualUnread).toBe(true);

    rerender(<ConversationContextMenu entry={entry()} anchor={ANCHOR} onClose={() => {}} />);
    expect(screen.getByTestId("conversation-menu-unread").textContent).toContain("标为已读");
    useChatStore.setState({ unreadByPeer: { alice: 2 } });
    fireEvent.click(screen.getByTestId("conversation-menu-unread"));
    expect(useConversationPrefsStore.getState().flags[key]?.manualUnread).toBe(false);
    expect(useChatStore.getState().unreadByPeer["alice"]).toBe(0);
  });

  it("真实未读 >0 时直接显「标为已读」", () => {
    render(
      <ConversationContextMenu
        entry={entry({ unread: 3 })}
        anchor={ANCHOR}
        onClose={() => {}}
      />,
    );
    expect(screen.getByTestId("conversation-menu-unread").textContent).toContain("标为已读");
  });

  it("删除写入 dismissedAt 并清旗标；删除项为 destructive 红", () => {
    useConversationPrefsStore.setState({
      flags: { [conversationKey("friend", "alice")]: { pinned: true, muted: true, manualUnread: true } },
    });
    render(<ConversationContextMenu entry={entry()} anchor={ANCHOR} onClose={() => {}} />);
    const del = screen.getByTestId("conversation-menu-delete");
    expect(del.className).toContain("text-destructive");
    fireEvent.click(del);
    const state = useConversationPrefsStore.getState();
    expect(state.dismissedAt[conversationKey("friend", "alice")]).toBeGreaterThan(0);
    expect(state.flags[conversationKey("friend", "alice")]).toBeUndefined();
  });

  it("Escape 关闭菜单（onClose 被调）", () => {
    const onClose = vi.fn();
    render(<ConversationContextMenu entry={entry()} anchor={ANCHOR} onClose={onClose} />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("菜单外右键关闭；菜单内右键不关闭", () => {
    const onClose = vi.fn();
    const { rerender } = render(
      <ConversationContextMenu entry={entry()} anchor={ANCHOR} onClose={onClose} />,
    );
    fireEvent.contextMenu(document.body);
    expect(onClose).toHaveBeenCalledTimes(1);
    rerender(<ConversationContextMenu entry={entry()} anchor={ANCHOR} onClose={onClose} />);
    fireEvent.contextMenu(screen.getByTestId("conversation-menu-pin"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
