import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { GuiConfig, NodeStatus } from "@/lib/ipc-types";

const { nodeStartMock } = vi.hoisted(() => ({ nodeStartMock: vi.fn() }));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    onNodeEvent: vi.fn(async () => () => {}),
    nodeStatus: vi.fn(),
    metricsGet: vi.fn(),
    nodeStart: nodeStartMock,
    nodeStop: vi.fn(),
  },
}));

import { useNodeStore } from "@/stores/node-store";
import { useNodeAutoStart } from "./use-node-auto-start";

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

const RUNNING: NodeStatus = { ...STOPPED, running: true, peerId: "self" };

function prime(over: {
  bootstrapPhase?: "idle" | "loading" | "ready" | "error";
  status?: NodeStatus | null;
  manualStopRequested?: boolean;
}): void {
  useNodeStore.setState({
    bootstrapPhase: "idle",
    status: null,
    manualStopRequested: false,
    autoStartRequested: false,
    autoStartPhase: "idle",
    autoStartError: null,
    ...over,
  });
}

beforeEach(() => {
  nodeStartMock.mockReset();
  prime({});
});

afterEach(() => {
  vi.restoreAllMocks();
});

// hook 只负责在 ready + 未运行时触发入口；闸门与压制语义在 store 内测。
describe("useNodeAutoStart", () => {
  it("ready 且未运行：触发 maybeAutoStart，成功后不重复触发", async () => {
    nodeStartMock.mockResolvedValue(RUNNING);
    prime({ bootstrapPhase: "ready", status: STOPPED });

    renderHook(() => useNodeAutoStart());

    await waitFor(() => {
      expect(nodeStartMock).toHaveBeenCalledTimes(1);
      expect(nodeStartMock).toHaveBeenCalledWith(CONFIG);
    });

    // status 翻转为运行中（来自启动结果），effect 重跑但闸门已消耗
    await act(async () => {
      useNodeStore.setState({ status: RUNNING });
    });
    await act(async () => {});
    expect(nodeStartMock).toHaveBeenCalledTimes(1);
  });

  it("已运行：不触发", async () => {
    prime({ bootstrapPhase: "ready", status: RUNNING });

    renderHook(() => useNodeAutoStart());
    await act(async () => {});

    expect(nodeStartMock).not.toHaveBeenCalled();
  });

  it("引导未就绪：不触发；就绪后再触发", async () => {
    nodeStartMock.mockResolvedValue(RUNNING);
    prime({ bootstrapPhase: "loading", status: null });

    const { rerender } = renderHook(() => useNodeAutoStart());
    await act(async () => {});
    expect(nodeStartMock).not.toHaveBeenCalled();

    prime({ bootstrapPhase: "ready", status: STOPPED });
    rerender();
    await waitFor(() => expect(nodeStartMock).toHaveBeenCalledTimes(1));
  });

  it("手动停止压制生效：hook 不再触发自动启动", async () => {
    prime({ bootstrapPhase: "ready", status: STOPPED, manualStopRequested: true });

    renderHook(() => useNodeAutoStart());
    await act(async () => {});

    expect(nodeStartMock).not.toHaveBeenCalled();
  });
});
