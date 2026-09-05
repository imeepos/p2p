import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import "@/i18n";
import type { EventsListCardProps } from "./events-list-card";
import { EventsListCard } from "./events-list-card";

const base: EventsListCardProps = {
  loading: false,
  linkFailed: false,
  onRetryLink: vi.fn(),
  bufferEmpty: false,
  filtered: [],
  onResetFilters: vi.fn(),
  locale: "zh-CN",
  expanded: new Set(),
  heightAt: () => 40,
  onToggle: vi.fn(),
};

describe("EventsListCard 空态与错误态", () => {
  it("订阅引导失败给显式错误文案与重试入口", () => {
    const onRetryLink = vi.fn();
    render(<EventsListCard {...base} linkFailed onRetryLink={onRetryLink} />);
    expect(screen.getByRole("alert")).toHaveTextContent("事件订阅未就绪");
    const retry = screen.getByRole("button", { name: "重试" });
    fireEvent.click(retry);
    expect(onRetryLink).toHaveBeenCalledTimes(1);
  });

  it("加载期渲染骨架行", () => {
    const { container } = render(<EventsListCard {...base} loading />);
    expect(
      container.querySelectorAll('[data-slot="skeleton"]').length,
    ).toBeGreaterThan(0);
  });

  it("过滤空态给一键清除筛选", () => {
    const onResetFilters = vi.fn();
    render(<EventsListCard {...base} onResetFilters={onResetFilters} />);
    expect(screen.getByText("没有匹配当前过滤条件的事件")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "清除筛选" }));
    expect(onResetFilters).toHaveBeenCalledTimes(1);
  });

  it("缓冲真空态不渲染清除筛选按钮", () => {
    render(<EventsListCard {...base} bufferEmpty />);
    expect(screen.getByText("暂无事件")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
  });
});
