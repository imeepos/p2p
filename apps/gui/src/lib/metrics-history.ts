import type { MetricsJson, MetricsPoint } from "./ipc-types";

// 契约 v2：5s 粒度、最近 120 点（10 分钟窗口），与后端窗口一致。
export const SAMPLE_INTERVAL_MS = 5000;
export const MAX_HISTORY_POINTS = 120;

export function metricsSnapshotPoint(
  metrics: MetricsJson,
  tMs = Date.now(),
): MetricsPoint {
  return {
    tMs,
    activeConnections: metrics.activeConnections,
    relaySessionsActive: metrics.relaySessionsActive,
    dialOkTotal:
      metrics.dialDirectOk + metrics.dialPunchOk + metrics.dialRelayOk,
    dialFailTotal:
      metrics.dialDirectFail + metrics.dialPunchFail + metrics.dialRelayFail,
  };
}

// 环形追加：新点在尾，超窗口弃最旧。
export function appendMetricsPoint(
  buffer: MetricsPoint[],
  point: MetricsPoint,
): MetricsPoint[] {
  const next = [...buffer, point];
  return next.length > MAX_HISTORY_POINTS
    ? next.slice(next.length - MAX_HISTORY_POINTS)
    : next;
}

function clampWalk(value: number, delta: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value + delta));
}

// mock 演示序列：随机游走，连接 0-8、会话 0-3、dial 计数单调、时间戳单调升序。
export function randomWalkHistory(now = Date.now()): MetricsPoint[] {
  const points: MetricsPoint[] = [];
  let connections = Math.floor(Math.random() * 5);
  let sessions = Math.floor(Math.random() * 2);
  let dialOk = 0;
  let dialFail = 0;
  for (let i = MAX_HISTORY_POINTS - 1; i >= 0; i -= 1) {
    connections = clampWalk(connections, Math.round((Math.random() - 0.5) * 3), 0, 8);
    sessions = clampWalk(sessions, Math.round((Math.random() - 0.5) * 1.6), 0, 3);
    dialOk += Math.round(Math.random() * 3);
    dialFail += Math.random() < 0.3 ? 1 : 0;
    points.push({
      tMs: now - i * SAMPLE_INTERVAL_MS,
      activeConnections: connections,
      relaySessionsActive: sessions,
      dialOkTotal: dialOk,
      dialFailTotal: dialFail,
    });
  }
  return points;
}
