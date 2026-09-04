import { beforeEach, describe, expect, it, vi } from "vitest";

const nodeState = vi.hoisted(() => ({
  status: null as {
    running: boolean;
    peerId: string | null;
    config: Record<string, unknown>;
  } | null,
  metrics: null as Record<string, unknown> | null,
  subscriptionLive: false,
  startNode: vi.fn(),
  stopNode: vi.fn(),
}));

vi.mock("@/stores/node-store", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/stores/node-store")>();
  return {
    ...actual,
    useNodeStore: { getState: () => nodeState, subscribe: vi.fn() },
  };
});

import { dashboardPage } from "./dashboard-page";
import { executePageAction } from "../page-registry";

beforeEach(() => {
  vi.clearAllMocks();
  nodeState.status = null;
  nodeState.metrics = null;
});

describe("dashboard 页 descriptor", () => {
  it("descriptor 快照与动作清单", () => {
    expect(dashboardPage.descriptor).toMatchSnapshot();
    expect(dashboardPage.descriptor.actions.map((a) => a.name)).toEqual([
      "start",
      "stop",
    ]);
  });

  it("state 与状态卡/指标卡同源", () => {
    nodeState.status = { running: true, peerId: "p1", config: {} };
    nodeState.metrics = { relaySessionsActive: 2 };
    const snapshot = dashboardPage.state?.() as Record<string, unknown>;
    expect(snapshot).toMatchObject({
      running: true,
      peerId: "p1",
      metrics: { relaySessionsActive: 2 },
    });
  });

  it("stop 缺 confirm 结构化拒绝且不触达 store", async () => {
    await expect(executePageAction("dashboard", "stop", {})).resolves.toMatchObject({
      ok: false,
      error: { code: "ACTION_CONFIRM_REQUIRED" },
    });
    expect(nodeState.stopNode).not.toHaveBeenCalled();
  });

  it("start 与启动按钮同源（status.config 传入 startNode）", async () => {
    nodeState.status = { running: false, peerId: "p1", config: { quicPort: 4001 } };
    nodeState.startNode.mockResolvedValue({ running: true });
    await expect(executePageAction("dashboard", "start", {})).resolves.toMatchObject({
      ok: true,
    });
    expect(nodeState.startNode).toHaveBeenCalledWith({ quicPort: 4001 });
  });

  it("start 在状态未加载时结构化失败（对齐按钮 disabled 语义）", async () => {
    await expect(executePageAction("dashboard", "start", {})).resolves.toMatchObject({
      ok: false,
      error: { code: "ACTION_FAILED" },
    });
    expect(nodeState.startNode).not.toHaveBeenCalled();
  });

  it("stop 带 confirm 与停止确认框同源", async () => {
    nodeState.stopNode.mockResolvedValue({ running: false });
    await expect(
      executePageAction("dashboard", "stop", { confirm: true }),
    ).resolves.toMatchObject({ ok: true });
    expect(nodeState.stopNode).toHaveBeenCalled();
  });
});
