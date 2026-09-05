import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import "@/i18n";
import { useNodeStore } from "@/stores/node-store";
import { DashboardMetricCards } from "./dashboard-metric-cards";

const METRICS = {
  dialDirectOk: 0,
  dialDirectFail: 0,
  dialPunchOk: 0,
  dialPunchFail: 0,
  dialRelayOk: 0,
  dialRelayFail: 0,
  addrDialFailures: 0,
  relayReconnects: 0,
  gateDenialsTotal: 0,
  activeConnections: 0,
  relaySessionsActive: 0,
};

describe("DashboardMetricCards 已知节点卡加载态", () => {
  it("首取数据未到时四张卡一律骨架，已知节点不得直接显示 0", () => {
    const { container } = render(<DashboardMetricCards metrics={null} />);
    expect(container.querySelectorAll('[data-slot="skeleton"]')).toHaveLength(
      4,
    );
    expect(screen.queryByText("0")).toBeNull();
  });

  it("数据到达后显示真实计数（0 是合法数据而非未知）", () => {
    useNodeStore.setState({ peers: {} });
    render(<DashboardMetricCards metrics={METRICS} />);
    expect(screen.getAllByText("0")).toHaveLength(4);
  });
});
