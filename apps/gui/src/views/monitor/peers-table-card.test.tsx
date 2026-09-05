import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import "@/i18n";
import type { PeersTableCardProps } from "./peers-table-card";
import { PeersTableCard } from "./peers-table-card";

function baseProps(over: Partial<PeersTableCardProps>): PeersTableCardProps {
  return {
    peers: [],
    bufferEmpty: true,
    nodeReady: true,
    nodeRunning: false,
    onStartNode: vi.fn(),
    onResetFilters: vi.fn(),
    locale: "zh-CN",
    now: Date.now(),
    onPing: () => () => Promise.resolve({ ok: true, rttMs: 1, hops: [], error: null }),
    onConnect: () => () => Promise.resolve({ peer: "p", hops: [], ok: true, totalMs: 1 }),
    onDisconnect: () => () => Promise.resolve(true),
    onShowDetail: vi.fn(),
    onOpenDial: vi.fn(),
    ...over,
  };
}

describe("PeersTableCard 空态分支", () => {
  it("状态未加载只给中性空态，不给任何动作", () => {
    render(<PeersTableCard {...baseProps({ nodeReady: false })} />);
    expect(screen.getByText("暂无已知节点")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("节点未运行时空态给启动引导而非注定失败的拨号", () => {
    const onStartNode = vi.fn();
    render(<PeersTableCard {...baseProps({ onStartNode })} />);
    expect(screen.getByText("节点未运行")).toBeInTheDocument();
    expect(
      screen.getByText("先启动节点，邻居才能被发现或拨号接入"),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "启动节点" }));
    expect(onStartNode).toHaveBeenCalledTimes(1);
  });

  it("节点运行中保留拨号入口", () => {
    const onOpenDial = vi.fn();
    render(
      <PeersTableCard {...baseProps({ nodeRunning: true, onOpenDial })} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "拨号添加节点" }));
    expect(onOpenDial).toHaveBeenCalledTimes(1);
  });

  it("过滤导致空态给一键清除筛选", () => {
    const onResetFilters = vi.fn();
    render(
      <PeersTableCard
        {...baseProps({
          peers: [],
          bufferEmpty: false,
          onResetFilters,
        })}
      />,
    );
    expect(screen.getByText("暂无数据")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "清除筛选" }));
    expect(onResetFilters).toHaveBeenCalledTimes(1);
  });
});
