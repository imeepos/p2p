import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import "@/i18n";
import type { MetricsJson } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";
import { HopStatsCard } from "./hop-stats-card";

function metrics(overrides: Partial<MetricsJson> = {}): MetricsJson {
  return {
    dialDirectOk: 0, dialDirectFail: 0, dialPunchOk: 0, dialPunchFail: 0,
    dialRelayOk: 0, dialRelayFail: 0, addrDialFailures: 0, relayReconnects: 0,
    gateDenialsTotal: 0, activeConnections: 0, relaySessionsActive: 0,
    ...overrides,
  };
}

beforeEach(() => {
  useNodeStore.setState({ metrics: null });
});

// 需求 7：ok/fail 两段按占比并排可见——fail>0 且 ok=0 时 fail 段满宽。
describe("HopStatsCard 逐跳比例条", () => {
  it("ok=0 fail>0：fail 段满宽可见，ok 段零宽（此前整条被裁成空白）", () => {
    useNodeStore.setState({
      metrics: metrics({ dialPunchOk: 0, dialPunchFail: 3 }),
    });
    render(<HopStatsCard />);
    const ok = screen.getByTestId("hop-punch-ok");
    const fail = screen.getByTestId("hop-punch-fail");
    expect(ok.style.width).toBe("0%");
    expect(fail.style.width).toBe("100%");
  });

  it("ok 与 fail 各半：两段 50%/50% 并排", () => {
    useNodeStore.setState({
      metrics: metrics({ dialRelayOk: 1, dialRelayFail: 1 }),
    });
    render(<HopStatsCard />);
    expect(screen.getByTestId("hop-relay-ok").style.width).toBe("50%");
    expect(screen.getByTestId("hop-relay-fail").style.width).toBe("50%");
  });

  it("ok>0 fail=0：ok 段满宽，fail 段零宽", () => {
    useNodeStore.setState({
      metrics: metrics({ dialPunchOk: 2, dialPunchFail: 0 }),
    });
    render(<HopStatsCard />);
    expect(screen.getByTestId("hop-punch-ok").style.width).toBe("100%");
    expect(screen.getByTestId("hop-punch-fail").style.width).toBe("0%");
  });

  it("零记录行保留空态文案与 aria 比例条语义", () => {
    useNodeStore.setState({ metrics: metrics() });
    render(<HopStatsCard />);
    expect(screen.getAllByText("暂无拨号记录")).toHaveLength(2);
    expect(screen.getAllByRole("img")).toHaveLength(2);
  });
});
