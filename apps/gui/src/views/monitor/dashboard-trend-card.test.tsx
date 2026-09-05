import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import "@/i18n";
import { DashboardTrendCard } from "./dashboard-trend-card";

describe("DashboardTrendCard 空态文案", () => {
  it("运行中无采样用独立说明，不与卡片头描述逐字重复", () => {
    render(<DashboardTrendCard history={[]} running />);
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
    render(<DashboardTrendCard history={[]} running={false} />);
    expect(screen.getByText("启动节点后开始采样")).toBeInTheDocument();
  });
});
