import { describe, expect, it } from "vitest";

import {
  appendMetricsPoint,
  MAX_HISTORY_POINTS,
  metricsSnapshotPoint,
  randomWalkHistory,
} from "./metrics-history";
import type { MetricsJson } from "./ipc-types";

const METRICS: MetricsJson = {
  dialDirectOk: 1, dialDirectFail: 2,
  dialPunchOk: 3, dialPunchFail: 4,
  dialRelayOk: 5, dialRelayFail: 6,
  addrDialFailures: 0,
  relayReconnects: 0,
  gateDenialsTotal: 0,
  activeConnections: 7,
  relaySessionsActive: 2,
};

describe("metrics-history", () => {
  it("snapshot point 聚合 dial 三跳 ok/fail 总数", () => {
    const point = metricsSnapshotPoint(METRICS, 1000);
    expect(point).toEqual({
      tMs: 1000,
      activeConnections: 7,
      relaySessionsActive: 2,
      dialOkTotal: 9,
      dialFailTotal: 12,
    });
  });

  it("append 环形缓冲封顶 120 点，新点在尾", () => {
    let buffer = Array.from({ length: MAX_HISTORY_POINTS }, (_, i) => ({
      tMs: i,
      activeConnections: 0,
      relaySessionsActive: 0,
      dialOkTotal: 0,
      dialFailTotal: 0,
    }));
    buffer = appendMetricsPoint(buffer, {
      tMs: 999999,
      activeConnections: 5,
      relaySessionsActive: 1,
      dialOkTotal: 1,
      dialFailTotal: 0,
    });
    expect(buffer.length).toBe(MAX_HISTORY_POINTS);
    expect(buffer[buffer.length - 1].tMs).toBe(999999);
    // 121 点弃最旧 1 个，剩下的最老是 tMs=1
    expect(buffer[0].tMs).toBe(1);
  });

  it("randomWalk 序列形状合法：120 点、时间戳单调、值域受约束", () => {
    const points = randomWalkHistory(1_000_000);
    expect(points.length).toBe(MAX_HISTORY_POINTS);
    for (let i = 1; i < points.length; i += 1) {
      expect(points[i].tMs).toBe(points[i - 1].tMs + 5000);
      expect(points[i].activeConnections).toBeGreaterThanOrEqual(0);
      expect(points[i].activeConnections).toBeLessThanOrEqual(8);
      expect(points[i].relaySessionsActive).toBeLessThanOrEqual(3);
      expect(points[i].dialOkTotal).toBeGreaterThanOrEqual(
        points[i - 1].dialOkTotal,
      );
    }
  });
});
