import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import "@/i18n";
import type { MetricsPoint } from "@/lib/ipc-types";
import { OverviewTrendCard } from "./overview-trend-card";

function point(activeConnections: number, relaySessionsActive: number): MetricsPoint {
  return { tMs: 1_757_200_000_000, activeConnections, relaySessionsActive, dialOkTotal: 0, dialFailTotal: 0 };
}

describe("OverviewTrendCard 空态文案", () => {
  it("运行中无采样用独立说明，不与卡片头描述逐字重复", () => {
    render(<OverviewTrendCard history={[]} running />);
    expect(screen.getByText("暂无趋势数据")).toBeInTheDocument();
    expect(
      screen.getByText("节点运行中但暂无有效采样，等待下一个采样点"),
    ).toBeInTheDocument();
    // 头部描述只出现一次（空态说明不再复用它）。
    expect(
      screen.getAllByText(
        "每 5 秒采样一个点，展示最近 120 点（10 分钟窗口）",
      ),
    ).toHaveLength(1);
  });

  it("未运行仍给启动引导说明", () => {
    render(<OverviewTrendCard history={[]} running={false} />);
    expect(screen.getByText("启动节点后开始采样")).toBeInTheDocument();
  });
});

// R2-18 回归：趋势区每个 sparkline svg 都必须 role + aria-label（指标名+当前值）。
describe("OverviewTrendCard sparkline 读屏可达（R2-18）", () => {
  it("每条趋势线带 role=img 且 aria-label 含指标名与当前值", () => {
    const history = [point(3, 1), point(4, 2), point(5, 2)];
    render(<OverviewTrendCard history={history} running />);
    const connections = screen.getByRole("img", { name: "活跃连接，当前 5" });
    expect(connections).toHaveAttribute("aria-label", "活跃连接，当前 5");
    expect(screen.getByRole("img", { name: "中继会话，当前 2" })).toBeInTheDocument();
    // 趋势区内不存在无 aria-label 的 svg（读屏不可达即违规）
    const svgs = document.querySelectorAll("svg");
    expect(svgs.length).toBeGreaterThan(0);
    for (const svg of svgs) {
      expect(svg).toHaveAttribute("aria-label");
    }
  });
});
