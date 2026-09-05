import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";

import type { GuiConfig, MetricsJson, NodeStatus } from "@/lib/ipc-types";

const { onNodeEventMock, nodeStatusMock, metricsGetMock } = vi.hoisted(() => ({
  onNodeEventMock: vi.fn(),
  nodeStatusMock: vi.fn(),
  metricsGetMock: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    onNodeEvent: onNodeEventMock,
    nodeStatus: nodeStatusMock,
    metricsGet: metricsGetMock,
  },
}));

import {
  hasDialableAddr,
  REFRESH_STALE_THRESHOLD,
  selectPeerCount,
  selectPeerList,
  useNodeStore,
  type NodeStoreState,
} from "./node-store";
import type { PeerEntry } from "./event-reducer";

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

const STATUS: NodeStatus = {
  running: false,
  peerId: "self",
  listenAddrs: [],
  uptimeSecs: 0,
  startedAtMs: null,
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

function okSubscription(): void {
  onNodeEventMock.mockImplementation(async (handler: (e: unknown) => void) => {
    handlerRef = handler;
    return () => {};
  });
}

let handlerRef: ((event: unknown) => void) | null = null;

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
  });
}

beforeEach(() => {
  onNodeEventMock.mockReset();
  nodeStatusMock.mockReset();
  metricsGetMock.mockReset();
  handlerRef = null;
  resetStore();
});

afterEach(() => {
  vi.restoreAllMocks();
});

const peer = (id: string, lastSeenMs: number): PeerEntry => ({
  peerId: id,
  addrs: [],
  source: "rendezvous",
  connected: true,
  lastSeenMs,
  hops: [],
});

const state = (peers: Record<string, PeerEntry>): NodeStoreState =>
  ({ peers } as unknown as NodeStoreState);

// 回归：useSyncExternalStore 按引用比较快照，selector 必须返回稳定引用。
describe("node-store selectors", () => {
  it("selectPeerList 同一 peers 引用返回同一数组（快照稳定）", () => {
    const s = state({ a: peer("a", 1) });
    expect(selectPeerList(s)).toBe(selectPeerList(s));
  });

  it("selectPeerList peers 变化时返回新数组且按 lastSeenMs 降序", () => {
    const s1 = state({ a: peer("a", 1), b: peer("b", 9) });
    const list1 = selectPeerList(s1);
    expect(list1.map((p) => p.peerId)).toEqual(["b", "a"]);

    const s2 = state({ a: peer("a", 5) });
    const list2 = selectPeerList(s2);
    expect(list2).not.toBe(list1);
    expect(list2.map((p) => p.peerId)).toEqual(["a"]);
  });

  it("selectPeerList 空 peers 引用稳定且计数为零", () => {
    const s = state({});
    expect(selectPeerList(s)).toBe(selectPeerList(s));
    expect(selectPeerList(s)).toHaveLength(0);
    expect(selectPeerCount(s)).toBe(0);
  });

  it("仅剩 loopback 地址的离线对端被隐藏（E5：rendezvous 泄漏废条目）", () => {
    const junk = { ...peer("junk", 3), connected: false, addrs: ["127.0.0.1/u60736", "::1/u40000"] };
    const lan = { ...peer("lan", 2), connected: false, addrs: ["192.168.1.5/u40001"] };
    const live = { ...peer("live", 1), connected: true, addrs: ["127.0.0.1/u40002"] };
    const s = state({ junk, lan, live });
    expect(selectPeerList(s).map((p) => p.peerId)).toEqual(["lan", "live"]);
    expect(selectPeerCount(s)).toBe(2);
  });

  it("hasDialableAddr 覆盖 loopback/链路本地/私网/公网", () => {
    expect(hasDialableAddr("127.0.0.1/u60736")).toBe(false);
    expect(hasDialableAddr("::1/u40000")).toBe(false);
    expect(hasDialableAddr("fe80::1/u40001")).toBe(false);
    expect(hasDialableAddr("localhost/u40002")).toBe(false);
    expect(hasDialableAddr("/u40003")).toBe(false);
    expect(hasDialableAddr("192.168.1.5/u40004")).toBe(true);
    expect(hasDialableAddr("203.0.113.7/t40005")).toBe(true);
    expect(hasDialableAddr("240e:abcd::1/u40006")).toBe(true);
  });
});

// UC monitor 数据可信：订阅/引导失败不得永挂骨架，失败后必须可重试；
// 周期刷新连败要标记数据可能过期；环形缓冲打满后增量计数仍精确。
describe("node-store bootstrap 数据链路", () => {
  it("订阅失败进入显式错误态，重试成功恢复", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    onNodeEventMock.mockRejectedValueOnce(new Error("sub boom"));
    await useNodeStore.getState().bootstrap();
    let s = useNodeStore.getState();
    expect(s.bootstrapPhase).toBe("error");
    expect(s.bootstrapError).toBe("sub boom");
    expect(s.subscriptionLive).toBe(false);

    okSubscription();
    nodeStatusMock.mockResolvedValue(STATUS);
    metricsGetMock.mockResolvedValue(METRICS);
    await useNodeStore.getState().bootstrap();
    s = useNodeStore.getState();
    expect(s.bootstrapPhase).toBe("ready");
    expect(s.subscriptionLive).toBe(true);
    expect(onNodeEventMock).toHaveBeenCalledTimes(2);
  });

  it("订阅成功但初始刷新失败同样进错误态；失败可重复重试且不重复订阅", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    okSubscription();
    nodeStatusMock.mockResolvedValue(STATUS);
    metricsGetMock
      .mockRejectedValueOnce(new Error("metrics down 1"))
      .mockRejectedValueOnce(new Error("metrics down 2"))
      .mockResolvedValue(METRICS);

    await useNodeStore.getState().bootstrap();
    expect(useNodeStore.getState().bootstrapPhase).toBe("error");
    expect(useNodeStore.getState().bootstrapError).toBe("metrics down 1");

    // 第一次重试仍失败——错误态必须允许再次重试，而不是锁死整会话。
    await useNodeStore.getState().bootstrap();
    expect(useNodeStore.getState().bootstrapPhase).toBe("error");
    expect(useNodeStore.getState().bootstrapError).toBe("metrics down 2");

    await useNodeStore.getState().bootstrap();
    expect(useNodeStore.getState().bootstrapPhase).toBe("ready");
    expect(onNodeEventMock).toHaveBeenCalledTimes(1);
  });

  it("周期刷新连败达阈值标记 dataStale，成功后自动恢复", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    okSubscription();
    nodeStatusMock.mockResolvedValue(STATUS);
    metricsGetMock.mockRejectedValue(new Error("refresh boom"));

    await useNodeStore.getState().bootstrap();
    for (let i = 0; i < REFRESH_STALE_THRESHOLD - 1; i += 1) {
      await useNodeStore.getState().refresh();
    }
    expect(useNodeStore.getState().consecutiveRefreshFailures).toBe(
      REFRESH_STALE_THRESHOLD,
    );
    expect(useNodeStore.getState().dataStale).toBe(true);
    expect(useNodeStore.getState().lastRefreshError).toBe("refresh boom");

    // 恢复成功：stale 即刻消失，横幅随之自动收起。
    metricsGetMock.mockResolvedValue(METRICS);
    await useNodeStore.getState().refresh();
    expect(useNodeStore.getState().dataStale).toBe(false);
    expect(useNodeStore.getState().consecutiveRefreshFailures).toBe(0);
  });

  it("未达阈值的零星失败不标记过期", async () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    okSubscription();
    nodeStatusMock.mockResolvedValue(STATUS);
    metricsGetMock.mockRejectedValueOnce(new Error("blip"));
    await useNodeStore.getState().bootstrap();
    expect(useNodeStore.getState().dataStale).toBe(false);
    expect(useNodeStore.getState().bootstrapPhase).toBe("error");
  });

  it("环形缓冲打满后 eventSeq 继续精确累加（暂停计数依据）", async () => {
    okSubscription();
    nodeStatusMock.mockResolvedValue(STATUS);
    metricsGetMock.mockResolvedValue(METRICS);
    await useNodeStore.getState().bootstrap();

    // 先灌满缓冲，模拟「暂停时缓冲已满」的真实挂机场景。
    for (let i = 0; i < 1000; i += 1) {
      handlerRef?.({ type: "node_stopped" });
    }
    const pausedSeq = useNodeStore.getState().eventSeq;

    // 暂停期间再来 50 条：旧的「live.length - snapshot.length」公式得 0，
    // 序号差必须精确等于 50。
    for (let i = 0; i < 50; i += 1) {
      handlerRef?.({ type: "node_stopped" });
    }
    const s = useNodeStore.getState();
    expect(s.events.length).toBe(1000);
    expect(s.eventSeq - pausedSeq).toBe(50);
  });
});
