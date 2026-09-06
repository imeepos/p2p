import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import "@/i18n";
import type { NodeEventType } from "@/lib/ipc-types";
import { ALL_EVENT_TYPES } from "@/views/network/event-meta";
import { countEventsByType } from "./event-filter-groups";
import { EventsFilterBar } from "./events-filter-bar";

const ALL = new Set<NodeEventType>(ALL_EVENT_TYPES);

function baseCounts() {
  return countEventsByType([]);
}

function setup(overrides: Partial<Parameters<typeof EventsFilterBar>[0]> = {}) {
  const onToggleType = vi.fn();
  const onResetFilters = vi.fn();
  const props = {
    query: "",
    onQueryChange: vi.fn(),
    errorOnly: false,
    onErrorOnlyChange: vi.fn(),
    typeFilter: ALL,
    onToggleType,
    counts: baseCounts(),
    onResetFilters,
    ...overrides,
  };
  render(<EventsFilterBar {...props} />);
  return { onToggleType, onResetFilters };
}

describe("EventsFilterBar 分组与计数（F20）", () => {
  it("五个语义分组全部渲染", () => {
    setup();
    for (const id of ["connection", "message", "group", "security", "node"]) {
      expect(screen.getByTestId(`event-filter-group-${id}`)).toBeInTheDocument();
    }
  });

  it("17 个类型 chip 全部存在", () => {
    setup();
    for (const type of ALL_EVENT_TYPES) {
      expect(screen.getByTestId(`event-filter-chip-${type}`)).toBeInTheDocument();
    }
  });

  it("空数据时计数为 0 且 chip 仍可切换", () => {
    const { onToggleType } = setup();
    const chip = screen.getByRole("button", { name: "发现，0 条" });
    expect(chip).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(chip);
    expect(onToggleType).toHaveBeenCalledWith("peer_discovered");
  });

  it("命中计数随缓冲区数据更新", () => {
    const events = [
      { type: "peer_discovered", peer: "a", addrs: ["x"], source: "rendezvous", tsMs: 1 },
      { type: "peer_discovered", peer: "b", addrs: ["y"], source: "mdns", tsMs: 2 },
      { type: "listen_failed", addr: "0.0.0.0/1", reason: "busy", tsMs: 3 },
    ] as Parameters<typeof countEventsByType>[0];
    setup({ counts: countEventsByType(events) });
    expect(screen.getByRole("button", { name: "发现，2 条" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "监听失败，1 条" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "连接，0 条" })).toBeInTheDocument();
  });

  it("默认无筛选时清除按钮禁用，有筛选时可用并回调", () => {
    const first = render(
      <EventsFilterBar
        query=""
        onQueryChange={vi.fn()}
        errorOnly={false}
        onErrorOnlyChange={vi.fn()}
        typeFilter={ALL}
        onToggleType={vi.fn()}
        counts={baseCounts()}
        onResetFilters={vi.fn()}
      />,
    );
    expect(screen.getByTestId("event-filter-clear")).toBeDisabled();
    first.unmount();

    const { onResetFilters } = setup({ query: "abc" });
    const clear = screen.getByTestId("event-filter-clear");
    expect(clear).toBeEnabled();
    fireEvent.click(clear);
    expect(onResetFilters).toHaveBeenCalledTimes(1);
  });
});
