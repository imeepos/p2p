// 会话列表压力测试（react-window 定高行路径）：3000 条会话下 DOM 行数有界、
// 滚动窗口移动、搜索过滤联动虚拟行、选中态跨滚动保持。
import { act, render } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent } from "@testing-library/react";

import { installJsdomVirtPolyfill } from "@/test/jsdom-virt";
import type { ConversationEntry } from "@/lib/conversation-entry";
import i18n from "@/i18n";

import { ConversationList } from "./conversation-list";

vi.mock("@/lib/ipc", () => ({ ipc: {} }));

const TOTAL = 3000;
const ROW_H = 64;

function entry(id: string, title: string): ConversationEntry {
  return {
    id,
    kind: "friend",
    title,
    subtitle: null,
    kindMark: { initial: "F", botIcon: false, groupBadge: false, dot: null },
    statusBadge: null,
    lastPreview: `预览 ${id}`,
    lastTsMs: Date.UTC(2026, 8, 10, 8, 0, 0),
    unread: 0,
    sendState: null,
    host: null,
    joinSeq: 0,
  };
}

function makeEntries(count: number): ConversationEntry[] {
  return Array.from({ length: count }, (_, i) => entry(`peer-${i}`, `会话 ${i}`));
}

function Harness({
  entries,
  selectedId,
  onSelect = () => {},
}: {
  entries: ConversationEntry[];
  selectedId: string | null;
  onSelect?: (entry: ConversationEntry) => void;
}): React.ReactElement {
  return (
    <div style={{ display: "flex", flexDirection: "column", height: 600 }}>
      <ConversationList
        entries={entries}
        selectedId={selectedId}
        loading={false}
        onSelect={onSelect}
      />
    </div>
  );
}

function visibleRows(container: HTMLElement): string[] {
  return [...container.querySelectorAll("[data-testid^='conversation-row-friend-']")].map(
    (el) => (el.getAttribute("data-testid") as string).replace("conversation-row-friend-", ""),
  );
}

beforeAll(() => {
  installJsdomVirtPolyfill();
});

beforeEach(async () => {
  await i18n.changeLanguage("zh-CN");
});

describe("ConversationList 压力测试（3000 会话，react-window 路径）", () => {
  it("超过阈值走虚拟路径，DOM 行数有界", async () => {
    const view = render(<Harness entries={makeEntries(TOTAL)} selectedId={null} />);
    const rows = visibleRows(view.container);
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.length).toBeLessThan(100);
    expect(rows[0]).toBe("peer-0");
    expect(visibleRows(view.container)).not.toContain(`peer-${TOTAL - 1}`);
  });

  it("滚动窗口移动且节点数仍有界", async () => {
    const view = render(<Harness entries={makeEntries(TOTAL)} selectedId={null} />);
    const scroller = view.container.querySelector<HTMLElement>(
      "[data-testid='conversation-items'] > div",
    )!;
    await act(async () => {
      Object.defineProperty(scroller, "scrollTop", {
        value: 2000 * ROW_H,
        writable: true,
        configurable: true,
      });
      scroller.dispatchEvent(new Event("scroll"));
      await new Promise((r) => setTimeout(r, 30));
    });
    const rows = visibleRows(view.container);
    expect(rows.length).toBeLessThan(100);
    expect(rows).toContain("peer-2000");
    expect(rows).not.toContain("peer-0");
  });

  it("搜索过滤联动虚拟行：过滤后仅剩匹配行", async () => {
    const entries = [...makeEntries(TOTAL), entry("peer-vip", "特别会话 VIP")];
    const view = render(<Harness entries={entries} selectedId={null} />);
    const input = view.container.querySelector<HTMLInputElement>(
      "[data-testid='conversation-search']",
    )!;
    await act(async () => {
      fireEvent.change(input, { target: { value: "特别会话" } });
      await new Promise((r) => setTimeout(r, 30));
    });
    const rows = visibleRows(view.container);
    expect(rows).toContain("peer-vip");
    expect(rows).not.toContain("peer-0");
  });

  it("选中态跨滚动保持；点击行回调 onSelect", async () => {
    const onSelect = vi.fn();
    const view = render(
      <Harness entries={makeEntries(TOTAL)} selectedId="peer-1" onSelect={onSelect} />,
    );
    expect(
      view
        .container.querySelector("[data-testid='conversation-row-friend-peer-1']")
        ?.getAttribute("aria-current"),
    ).toBe("true");

    const row0 = view.container.querySelector<HTMLElement>(
      "[data-testid='conversation-row-friend-peer-0']",
    )!;
    await act(async () => {
      fireEvent.click(row0);
    });
    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ id: "peer-0" }));
  });
});
