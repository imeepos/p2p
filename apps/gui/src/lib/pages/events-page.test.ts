import { beforeEach, describe, expect, it, vi } from "vitest";

const setStateMock = vi.hoisted(() => vi.fn());
const exportMock = vi.hoisted(() => vi.fn());
const nodeState = vi.hoisted(() => ({
  events: [] as Array<Record<string, unknown>>,
  subscriptionLive: true,
}));

vi.mock("@/lib/ipc", () => ({ ipc: {} }));
vi.mock("@/views/monitor/events-export", () => ({ exportEventsJson: exportMock }));
vi.mock("@/stores/node-store", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/stores/node-store")>();
  return {
    ...actual,
    useNodeStore: { getState: () => nodeState, setState: setStateMock },
  };
});

import { eventsPage } from "./events-page";
import { executePageAction } from "../page-registry";

beforeEach(() => {
  vi.clearAllMocks();
  nodeState.events = [];
  nodeState.subscriptionLive = true;
});

describe("events 页 descriptor", () => {
  it("descriptor 快照与动作清单", () => {
    expect(eventsPage.descriptor).toMatchSnapshot();
    expect(eventsPage.descriptor.actions.map((a) => a.name)).toEqual([
      "clear",
      "export",
    ]);
  });

  it("state 与事件列表同源（subscriptionLive + 缓冲行）", () => {
    nodeState.events = [{ type: "peer_discovered" }, { type: "dial_failed" }];
    const snapshot = eventsPage.state?.() as {
      subscriptionLive: boolean;
      total: number;
      latest: unknown[];
    };
    expect(snapshot).toMatchObject({ subscriptionLive: true, total: 2 });
    expect(snapshot.latest).toHaveLength(2);
  });

  it("clear 缺 confirm 结构化拒绝且不触达 store", async () => {
    await expect(executePageAction("events", "clear", {})).resolves.toMatchObject({
      ok: false,
      error: { code: "ACTION_CONFIRM_REQUIRED" },
    });
    expect(setStateMock).not.toHaveBeenCalled();
  });

  it("clear 带 confirm 直写 store 清空缓冲（与确认框同源）", async () => {
    const result = await executePageAction("events", "clear", { confirm: true });
    expect(result).toMatchObject({ ok: true, data: { cleared: true } });
    expect(setStateMock).toHaveBeenCalledWith({ events: [] });
  });

  it("export 与导出按钮同源（exportEventsJson 当前缓冲）", async () => {
    nodeState.events = [{ type: "peer_discovered" }];
    const result = await executePageAction("events", "export", {});
    expect(result).toMatchObject({ ok: true, data: { exported: 1 } });
    expect(exportMock).toHaveBeenCalledWith([{ type: "peer_discovered" }]);
  });
});
