// agent 会话段两级树测试（T5 移植验收矩阵）：host 键分组/未分组尾组/折叠开合/
// 当前组高亮+强制展开/折叠态行内限流「展开 N 个」/行点击与右键上抛。
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { resetWorkspaceUiForTest } from "@/acp/workspace-ui-store";
import type { ConversationEntry } from "@/lib/conversation-entry";

import { AgentSessionTree } from "./agent-session-tree";

await import("@/i18n");

function agentEntry(
  id: string,
  title: string,
  host: string | null,
): ConversationEntry {
  return {
    id,
    kind: "agent",
    title,
    subtitle: null,
    kindMark: { initial: null, botIcon: true, groupBadge: false, dot: null },
    statusBadge: null,
    lastPreview: null,
    lastTsMs: 0,
    unread: 0,
    sendState: null,
    host,
    joinSeq: 0,
  };
}

function renderTree(options: {
  entries: ConversationEntry[];
  selectedId?: string | null;
}) {
  const onSelect = vi.fn();
  const onRowContextMenu = vi.fn();
  render(
    <AgentSessionTree
      entries={options.entries}
      selectedId={options.selectedId ?? null}
      onSelect={onSelect}
      mutedOf={() => false}
      onRowContextMenu={onRowContextMenu}
    />,
  );
  return { onSelect, onRowContextMenu };
}

beforeEach(() => {
  resetWorkspaceUiForTest();
});

describe("AgentSessionTree 两级树", () => {
  it("按 host 分组渲染组头；无 host 条目落未分组尾组（I1）", () => {
    renderTree({
      entries: [
        agentEntry("a1", "甲", "127.0.0.1:8787"),
        agentEntry("a2", "乙", "10.0.0.2:9000"),
        agentEntry("a3", "丙", null),
      ],
    });
    expect(screen.getByTestId("acp-group-row-127.0.0.1:8787")).toBeTruthy();
    expect(screen.getByTestId("acp-group-row-10.0.0.2:9000")).toBeTruthy();
    const heads = screen.getAllByTestId(/^acp-group-row-/);
    expect(heads[heads.length - 1].dataset.testid).toBe("acp-group-row-ungrouped");
    expect(heads[heads.length - 1].textContent).toContain("未分组");
  });

  it("点组头折叠/展开：折叠后子行消失、其余组不受影响（I3）", () => {
    renderTree({
      entries: [agentEntry("a1", "甲", "h1"), agentEntry("a2", "乙", "h2")],
    });
    expect(screen.getByTestId("conversation-row-agent-a1")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-group-row-h1"));
    expect(screen.queryByTestId("conversation-row-agent-a1")).toBeNull();
    expect(screen.getByTestId("acp-group-row-h1").getAttribute("aria-expanded")).toBe("false");
    expect(screen.getByTestId("conversation-row-agent-a2")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-group-row-h1"));
    expect(screen.getByTestId("conversation-row-agent-a1")).toBeTruthy();
  });

  it("含当前会话的组 folder 高亮且强制展开：折叠操作不收敛选中组（I2/I3）", () => {
    renderTree({
      entries: [agentEntry("a1", "甲", "h1"), agentEntry("a2", "乙", "h2")],
      selectedId: "a2",
    });
    const folder = screen.getByTestId("acp-group-row-h2").querySelector("svg");
    expect(folder?.className.baseVal).toContain("text-info");
    fireEvent.click(screen.getByTestId("acp-group-row-h2"));
    expect(screen.getByTestId("conversation-row-agent-a2")).toBeTruthy();
  });

  it("折叠态行内限流：每组显 5 条 + 「展开 N 个」一次性全开（I4）", () => {
    renderTree({
      entries: Array.from({ length: 7 }, (_, i) => agentEntry(`a${i}`, `agent${i}`, "h1")),
    });
    expect(screen.getAllByTestId(/^conversation-row-agent-/)).toHaveLength(5);
    const expand = screen.getByTestId("acp-group-expand-h1");
    expect(expand.textContent).toContain("2");
    fireEvent.click(expand);
    expect(screen.getAllByTestId(/^conversation-row-agent-/)).toHaveLength(7);
    expect(screen.queryByTestId("acp-group-expand-h1")).toBeNull();
  });

  it("行点击上抛 onSelect；右键上抛条目与坐标", () => {
    const { onSelect, onRowContextMenu } = renderTree({
      entries: [agentEntry("a1", "甲", "h1")],
    });
    const row = screen.getByTestId("conversation-row-agent-a1");
    fireEvent.click(row);
    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ id: "a1" }));
    fireEvent.contextMenu(row);
    expect(onRowContextMenu).toHaveBeenCalledWith(
      expect.objectContaining({ id: "a1" }),
      expect.objectContaining({ x: expect.any(Number), y: expect.any(Number) }),
    );
  });
});
