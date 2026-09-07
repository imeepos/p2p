import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useMemo } from "react";

import "@/i18n";
import type { ConversationEntry } from "@/lib/conversation-entry";
import { applyConversationPrefs } from "@/lib/conversation-overlay";
import { useConversationPrefsStore } from "@/stores/conversation-prefs-store";
import { ConversationRow } from "./conversation-row";
import { ConversationList } from "./conversation-list";

function entry(id: string, overrides?: Partial<ConversationEntry>): ConversationEntry {
  return {
    id,
    kind: "friend",
    title: id,
    subtitle: null,
    kindMark: { initial: id[0] ?? "?", botIcon: false, groupBadge: false, dot: null },
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

beforeEach(() => {
  localStorage.clear();
  useConversationPrefsStore.setState({ flags: {}, dismissedAt: {} });
});

/** 页面管道同款：entries 先经偏好覆盖层再进列表（use-conversation-entries 的角色） */
function OverlayedList(props: {
  entries: ConversationEntry[];
  selectedId: string | null;
  onSelect: (entry: ConversationEntry) => void;
}) {
  const flags = useConversationPrefsStore((s) => s.flags);
  const dismissedAt = useConversationPrefsStore((s) => s.dismissedAt);
  const visible = useMemo(
    () => applyConversationPrefs(props.entries, { flags, dismissedAt }),
    [props.entries, flags, dismissedAt],
  );
  return <ConversationList entries={visible} selectedId={props.selectedId} loading={false} onSelect={props.onSelect} />;
}

describe("ConversationRow 右键事件与免打扰标示", () => {
  it("contextmenu 事件上抛坐标且不弹浏览器默认菜单", () => {
    const onContextMenu = vi.fn();
    render(<ConversationRow entry={entry("alice")} active={false} onSelect={() => {}} onContextMenu={onContextMenu} />);
    const row = screen.getByTestId("conversation-row-friend-alice");
    fireEvent.contextMenu(row, { clientX: 11, clientY: 22 });
    expect(onContextMenu).toHaveBeenCalledTimes(1);
    expect((onContextMenu.mock.calls[0]?.[0] as MouseEvent).clientX).toBe(11);
  });

  it("muted 时渲染静音铃（aria 可达）", () => {
    render(<ConversationRow entry={entry("alice")} active={false} onSelect={() => {}} muted />);
    expect(screen.getByTestId("conversation-muted-alice")).toBeTruthy();
    expect(screen.getByLabelText("已开启消息免打扰")).toBeTruthy();
  });
});

describe("ConversationList 集成：右键弹出菜单", () => {
  function renderList(entries: ConversationEntry[], onSelect?: (entry: ConversationEntry) => void) {
    return render(
      <OverlayedList entries={entries} selectedId={null} onSelect={onSelect ?? (() => {})} />,
    );
  }

  it("行上右键出现六项菜单（portal 渲染）", () => {
    renderList([entry("alice")]);
    fireEvent.contextMenu(screen.getByTestId("conversation-row-friend-alice"));
    expect(screen.getByTestId("conversation-context-menu")).toBeTruthy();
    expect(screen.getByTestId("conversation-menu-delete").textContent).toContain("删除");
  });

  it("删除后行从列表消失；新消息（lastTsMs 推进）后回归", () => {
    const { rerender } = renderList([entry("alice")]);
    fireEvent.contextMenu(screen.getByTestId("conversation-row-friend-alice"));
    fireEvent.click(screen.getByTestId("conversation-menu-delete"));
    expect(screen.queryByTestId("conversation-row-friend-alice")).toBeNull();

    rerender(
      <OverlayedList
        entries={[entry("alice", { lastTsMs: Date.now() + 1 })]}
        selectedId={null}
        onSelect={() => {}}
      />,
    );
    expect(screen.getByTestId("conversation-row-friend-alice")).toBeTruthy();
  });

  it("标为未读后行上出现未读角标；点行上抛 onSelect（旗标由页面层清）", () => {
    const onSelect = vi.fn();
    renderList([entry("alice")], onSelect);
    fireEvent.contextMenu(screen.getByTestId("conversation-row-friend-alice"));
    fireEvent.click(screen.getByTestId("conversation-menu-unread"));
    expect(screen.getByTestId("conversation-unread-alice")).toBeTruthy();

    fireEvent.click(screen.getByTestId("conversation-row-friend-alice"));
    expect(onSelect).toHaveBeenCalledTimes(1);
    // 列表层不清旗标（chat-page selectEntry 负责）；此处旗标仍在、角标仍在
    expect(useConversationPrefsStore.getState().flags["friend:alice"]?.manualUnread).toBe(true);
    expect(screen.getByTestId("conversation-unread-alice")).toBeTruthy();
  });
});
