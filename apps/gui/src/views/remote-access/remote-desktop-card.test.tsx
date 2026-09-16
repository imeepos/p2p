import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const startMock = vi.fn(async (requireApproval: boolean) => ({
  running: true,
  requireApproval,
  sessionCount: 0,
  pendingApprovals: [],
  fps: 15,
}));
const stopMock = vi.fn(async () => ({
  running: false,
  requireApproval: true,
  sessionCount: 0,
  pendingApprovals: [],
  fps: 15,
}));
const approveMock = vi.fn(async (_peer: string) => true);
const denyMock = vi.fn(async (_peer: string) => true);
const qualityMock = vi.fn(async (fps: number) => ({
  running: true,
  requireApproval: true,
  sessionCount: 0,
  pendingApprovals: [],
  fps,
}));
const connectMock = vi.fn(async (_peer: string, _sessionId?: string) => ({
  connected: true,
  sessionId: "0123456789abcdef",
}));
const closeMock = vi.fn(async () => ({ connected: false, sessionId: null }));
const statusMock = vi.fn(async () => ({
  running: true,
  requireApproval: true,
  sessionCount: 0,
  pendingApprovals: [],
  fps: 15,
}));
const viewerStatusMock = vi.fn(async () => ({ connected: false, sessionId: null }));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    rdHostStart: (requireApproval: boolean) => startMock(requireApproval),
    rdHostStop: () => stopMock(),
    rdHostStatus: () => statusMock(),
    rdApprove: (peer: string) => approveMock(peer),
    rdDeny: (peer: string) => denyMock(peer),
    rdQualitySet: (fps: number) => qualityMock(fps),
    rdViewerConnect: (peer: string, sessionId?: string) =>
      connectMock(peer, sessionId),
    rdViewerClose: () => closeMock(),
    rdViewerStatus: () => viewerStatusMock(),
  },
}));

vi.mock("@/stores/node-store", () => ({
  useNodeStore: (selector?: (s: unknown) => unknown) => {
    const state = { status: { running: true, peerId: "SelfPeer" } };
    return selector ? selector(state) : state;
  },
}));

import "@/i18n";
import { RemoteDesktopCard } from "./remote-desktop-card";
import type { UseRdPageModel } from "./use-rd-model";

function idleModel(): UseRdPageModel {
  return {
    running: true,
    host: {
      running: false,
      requireApproval: true,
      sessionCount: 0,
      pendingApprovals: [],
      fps: 15,
    },
    viewer: { connected: false, sessionId: null },
    busy: null,
    lastError: null,
    startHost: async (requireApproval: boolean) => {
      void startMock(requireApproval);
    },
    stopHost: async () => {
      void stopMock();
    },
    approve: async (peer: string) => {
      void approveMock(peer);
    },
    deny: async (peer: string) => {
      void denyMock(peer);
    },
    setQuality: async (fps: number) => {
      void qualityMock(fps);
    },
    connectViewer: async (peer: string) => {
      void connectMock(peer);
    },
    closeViewer: async () => {
      void closeMock();
    },
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("RemoteDesktopCard", () => {
  it("启停被控端服务并回读状态", async () => {
    render(<RemoteDesktopCard model={idleModel()} />);
    const startBtn = screen.getByRole("button", {
      name: "开启被控",
    });
    fireEvent.click(startBtn);
    await waitFor(() => expect(startMock).toHaveBeenCalledWith(true));
    // 审批开关默认开：启动参数携带 requireApproval=true
    expect(startMock.mock.calls[0][0]).toBe(true);
  });

  it("待审批队列渲染并可批准/拒绝", async () => {
    const model = idleModel();
    model.host = {
      running: true,
      requireApproval: true,
      sessionCount: 0,
      pendingApprovals: ["PeerA1111111111111111111"],
      fps: 15,
    };
    render(<RemoteDesktopCard model={model} />);
    expect(screen.getByText("PeerA1111111111111111111")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "批准" }));
    await waitFor(() =>
      expect(approveMock).toHaveBeenCalledWith("PeerA1111111111111111111"),
    );
    fireEvent.click(screen.getByRole("button", { name: "拒绝" }));
    await waitFor(() =>
      expect(denyMock).toHaveBeenCalledWith("PeerA1111111111111111111"),
    );
  });

  it("viewer 连接表单与断开", async () => {
    const model = idleModel();
    render(<RemoteDesktopCard model={model} />);
    const peerInput = screen.getByPlaceholderText("PeerId（base58）");
    fireEvent.change(peerInput, { target: { value: "PeerB2222222222222222" } });
    fireEvent.click(screen.getByRole("button", { name: "连接" }));
    await waitFor(() =>
      expect(connectMock).toHaveBeenCalledWith("PeerB2222222222222222"),
    );
    // 连接后显示断开按钮
    const model2 = idleModel();
    model2.viewer = { connected: true, sessionId: "0123456789abcdef" };
    render(<RemoteDesktopCard model={model2} />);
    fireEvent.click(screen.getByRole("button", { name: "断开" }));
    await waitFor(() => expect(closeMock).toHaveBeenCalled());
  });
});