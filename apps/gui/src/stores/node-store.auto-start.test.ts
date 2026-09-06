import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { GuiConfig, MetricsJson, NodeStatus } from "@/lib/ipc-types";

const {
  onNodeEventMock,
  nodeStatusMock,
  metricsGetMock,
  nodeStartMock,
  nodeStopMock,
} = vi.hoisted(() => ({
  onNodeEventMock: vi.fn(),
  nodeStatusMock: vi.fn(),
  metricsGetMock: vi.fn(),
  nodeStartMock: vi.fn(),
  nodeStopMock: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    onNodeEvent: onNodeEventMock,
    nodeStatus: nodeStatusMock,
    metricsGet: metricsGetMock,
    nodeStart: nodeStartMock,
    nodeStop: nodeStopMock,
  },
}));

import { useNodeStore } from "./node-store";

const CONFIG: GuiConfig = {
  quicPort: 0,
  tcpPort: 0,
  enableMdns: true,
  dataDir: "/tmp",
  bootstrap: [],
  relayAddrs: [],
  advertisedAddrs: [],
  observationPort: null,
  observationAddrs: [],
};

const STOPPED: NodeStatus = {
  running: false,
  peerId: null,
  listenAddrs: [],
  uptimeSecs: 0,
  startedAtMs: null,
  config: CONFIG,
};

const RUNNING: NodeStatus = {
  running: true,
  peerId: "self",
  listenAddrs: ["0.0.0.0/u34000"],
  uptimeSecs: 1,
  startedAtMs: 1,
  config: CONFIG,
};

const METRICS: MetricsJson = {
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

function resetStore(): void {
  useNodeStore.setState({
    status: null,
    metrics: null,
    metricsHistory: [],
    peers: {},
    events: [],
    eventSeq: 0,
    subscriptionLive: false,
    bootstrapPhase: "idle",
    bootstrapError: null,
    dataStale: false,
    consecutiveRefreshFailures: 0,
    lastRefreshError: null,
    autoStartPhase: "idle",
    autoStartError: null,
    autoStartRequested: false,
    manualStopRequested: false,
  });
}

beforeEach(() => {
  for (const m of [onNodeEventMock, nodeStatusMock, metricsGetMock, nodeStartMock, nodeStopMock]) {
    m.mockReset();
  }
  onNodeEventMock.mockImplementation(async () => () => {});
  nodeStatusMock.mockResolvedValue(STOPPED);
  metricsGetMock.mockResolvedValue(METRICS);
  resetStore();
});

afterEach(() => {
  vi.restoreAllMocks();
});

// UX1 启动即在线：引导完成且未运行则自动启动一次；手动停止意愿优先；
// 失败显式可观测且重试可反复触发。语义红线见 gui-coordination UX1 行。
describe("node-store auto-start（UX1 启动即在线）", () => {
  it("引导完成后节点未运行：自动启动恰一次，且用状态携带的配置快照", async () => {
    nodeStartMock.mockResolvedValue(RUNNING);

    await useNodeStore.getState().bootstrap();
    await useNodeStore.getState().maybeAutoStart();

    expect(nodeStartMock).toHaveBeenCalledTimes(1);
    expect(nodeStartMock).toHaveBeenCalledWith(CONFIG);
    expect(useNodeStore.getState().status?.running).toBe(true);
    expect(useNodeStore.getState().autoStartPhase).toBe("idle");
    expect(useNodeStore.getState().autoStartRequested).toBe(true);

    // 周期刷新/重新引导再次进入触发入口：闸门拦截，不重启
    await useNodeStore.getState().maybeAutoStart();
    await useNodeStore.getState().bootstrap();
    await useNodeStore.getState().maybeAutoStart();
    expect(nodeStartMock).toHaveBeenCalledTimes(1);
  });

  it("节点已运行：不触发自动启动，闸门未被消耗", async () => {
    nodeStatusMock.mockResolvedValue(RUNNING);
    nodeStartMock.mockResolvedValue(RUNNING);

    await useNodeStore.getState().bootstrap();
    await useNodeStore.getState().maybeAutoStart();

    expect(nodeStartMock).not.toHaveBeenCalled();
    expect(useNodeStore.getState().autoStartRequested).toBe(false);
  });

  it("自动启动失败：显式 failed 态与错误文本；重试可反复触发直至成功", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    nodeStartMock
      .mockRejectedValueOnce(new Error("port busy"))
      .mockRejectedValueOnce(new Error("port busy again"))
      .mockResolvedValue(RUNNING);

    await useNodeStore.getState().bootstrap();
    await useNodeStore.getState().maybeAutoStart();
    let s = useNodeStore.getState();
    expect(s.autoStartPhase).toBe("failed");
    expect(s.autoStartError).toBe("port busy");
    expect(s.status?.running).toBe(false);

    await useNodeStore.getState().retryAutoStart();
    s = useNodeStore.getState();
    expect(s.autoStartPhase).toBe("failed");
    expect(s.autoStartError).toBe("port busy again");

    await useNodeStore.getState().retryAutoStart();
    s = useNodeStore.getState();
    expect(s.autoStartPhase).toBe("idle");
    expect(s.autoStartError).toBeNull();
    expect(s.status?.running).toBe(true);
    expect(nodeStartMock).toHaveBeenCalledTimes(3);
  });

  it("手动停止后：本轮内周期刷新与重新引导均不复活自动启动", async () => {
    nodeStartMock.mockResolvedValue(RUNNING);
    nodeStopMock.mockResolvedValue(STOPPED);

    await useNodeStore.getState().bootstrap();
    await useNodeStore.getState().maybeAutoStart();
    expect(useNodeStore.getState().status?.running).toBe(true);

    await useNodeStore.getState().stopNode();
    expect(useNodeStore.getState().manualStopRequested).toBe(true);

    nodeStartMock.mockClear();
    nodeStatusMock.mockResolvedValue(STOPPED);
    await useNodeStore.getState().refresh();
    await useNodeStore.getState().maybeAutoStart();
    await useNodeStore.getState().maybeAutoStart();
    await useNodeStore.getState().bootstrap();
    await useNodeStore.getState().maybeAutoStart();
    expect(nodeStartMock).not.toHaveBeenCalled();
  });

  it("闸门未消耗时手动停止（用户抢先启停）：此后不再自动启动", async () => {
    nodeStartMock.mockResolvedValue(RUNNING);
    nodeStopMock.mockResolvedValue(STOPPED);

    await useNodeStore.getState().bootstrap();
    await useNodeStore.getState().startNode(CONFIG);
    await useNodeStore.getState().stopNode();

    nodeStartMock.mockClear();
    nodeStatusMock.mockResolvedValue(STOPPED);
    await useNodeStore.getState().maybeAutoStart();
    expect(nodeStartMock).not.toHaveBeenCalled();
    expect(useNodeStore.getState().manualStopRequested).toBe(true);
  });
});
