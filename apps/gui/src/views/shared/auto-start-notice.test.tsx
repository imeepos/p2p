import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

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

import "@/i18n";
import { useNodeStore } from "@/stores/node-store";
import { AutoStartNotice } from "./auto-start-notice";

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

function reset(
  over: Partial<{
    autoStartPhase: "idle" | "starting" | "failed";
    autoStartError: string | null;
    status: NodeStatus | null;
    manualStopRequested: boolean;
  }> = {},
): void {
  useNodeStore.setState({
    autoStartPhase: "idle",
    autoStartError: null,
    status: STOPPED,
    manualStopRequested: false,
    ...over,
  });
}

beforeEach(() => {
  nodeStartMock.mockReset();
  reset();
});

describe("AutoStartNotice", () => {
  it("idle 不渲染任何横幅", () => {
    const { container } = render(<AutoStartNotice />);
    expect(container).toBeEmptyDOMElement();
  });

  it("starting 呈现可感知的启动中状态条", () => {
    reset({ autoStartPhase: "starting" });
    render(<AutoStartNotice />);
    expect(screen.getByRole("status")).toBeInTheDocument();
    expect(screen.getByText("正在自动启动节点…")).toBeInTheDocument();
  });

  it("failed 呈现显式错误与重试入口，点击走真实重试直至成功", async () => {
    nodeStartMock.mockRejectedValueOnce(new Error("port busy"));
    reset({ autoStartPhase: "failed", autoStartError: "port busy" });

    const { rerender } = render(<AutoStartNotice />);
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByText("节点自动启动失败")).toBeInTheDocument();
    expect(screen.getByText("port busy")).toBeInTheDocument();

    // 第一次重试仍失败：错误态必须保持显式，不得静默消失
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    await waitFor(() => {
      expect(useNodeStore.getState().autoStartError).toBe("port busy");
    });
    expect(nodeStartMock).toHaveBeenCalledTimes(1);

    // 第二次重试成功：横幅随 phase 归位自然消失
    nodeStartMock.mockResolvedValueOnce({ ...STOPPED, running: true });
    fireEvent.click(screen.getByRole("button", { name: "重试" }));
    await waitFor(() => {
      expect(useNodeStore.getState().autoStartPhase).toBe("idle");
    });
    rerender(<AutoStartNotice />);
    expect(screen.queryByRole("alert")).toBeNull();
    expect(useNodeStore.getState().status?.running).toBe(true);
  });
});
