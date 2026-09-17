import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { encodeRdFrame } from "@/lib/rd-frame";

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
const mouseMock = vi.fn(
  async (_x: number, _y: number, _b: number, _dx: number, _dy: number) => true,
);
const keyMock = vi.fn(
  async (_code: number, _down: boolean, _mods: number) => true,
);
const resetKeysMock = vi.fn(async () => true);
const configGetMock = vi.fn();

vi.mock("@/lib/ipc", () => ({
  ipc: {
    configGet: () => configGetMock(),
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
    onFrame: () => () => {},
    sendMouse: async (x: number, y: number, b: number, dx: number, dy: number) => {
      void mouseMock(x, y, b, dx, dy);
      return true;
    },
    sendKey: async (code: number, down: boolean, mods: number) => {
      void keyMock(code, down, mods);
      return true;
    },
    resetKeys: async () => {
      void resetKeysMock();
      return true;
    },
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  // 默认无契约字段：卡片初值回退内置 true/15（既有断言口径不变）。
  configGetMock.mockResolvedValue({});
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
    // 行首 PeerNameCell：非好友显缩略 ID（6+4 口径），title 悬挂全文
    const cell = screen.getByTitle("PeerA1111111111111111111");
    expect(cell).toHaveTextContent("PeerA1…1111");
    // 批准/拒绝走 AsyncButton（防重入），动作仍携带完整 peerId
    fireEvent.click(screen.getByTestId("rd-approve-PeerA1111111111111111111"));
    await waitFor(() =>
      expect(approveMock).toHaveBeenCalledWith("PeerA1111111111111111111"),
    );
    fireEvent.click(screen.getByTestId("rd-deny-PeerA1111111111111111111"));
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

  it("连接后渲染画布并下发鼠标/键盘/释放事件", async () => {
    const model = idleModel();
    model.viewer = { connected: true, sessionId: "0123456789abcdef" };
    let sink: ((buf: ArrayBuffer) => void) | null = null;
    model.onFrame = (cb) => {
      sink = cb;
      return () => {};
    };
    const { unmount } = render(<RemoteDesktopCard model={model} />);
    const canvas = screen.getByTestId("rd-frame-canvas");
    canvas.getBoundingClientRect = () =>
      ({
        left: 0,
        top: 0,
        width: 320,
        height: 180,
        right: 320,
        bottom: 180,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      }) as DOMRect;
    // 首帧设置画布尺寸（jsdom 无 2d ctx，跳过绘制但保留坐标映射）
    act(() => {
      sink?.(encodeRdFrame(320, 180, 1, new Uint8ClampedArray(320 * 180 * 4)));
    });
    fireEvent.pointerDown(canvas, {
      clientX: 160,
      clientY: 90,
      buttons: 1,
      button: 0,
    });
    await waitFor(() => expect(mouseMock).toHaveBeenCalled());
    expect(mouseMock.mock.calls[0].slice(0, 2)).toEqual([160, 90]);
    fireEvent.keyDown(canvas, { code: "KeyA", shiftKey: true });
    await waitFor(() =>
      expect(keyMock).toHaveBeenCalledWith(0x04, true, 0x1),
    );
    fireEvent.blur(canvas);
    await waitFor(() => expect(resetKeysMock).toHaveBeenCalled());
    unmount();
  });
});

describe("RemoteDesktopCard 配置默认值（config-centralization W1）", () => {
  it("挂载读配置：approvalOn/fps 取 rdRequireApproval/rdFps", async () => {
    configGetMock.mockResolvedValue({ rdRequireApproval: false, rdFps: 30 });
    render(<RemoteDesktopCard model={idleModel()} />);
    const fpsInput = screen.getByLabelText("帧率（fps）") as HTMLInputElement;
    await waitFor(() => expect(fpsInput.value).toBe("30"));
    expect(
      screen.getByRole("switch", { name: "新会话需审批" }),
    ).toHaveAttribute("aria-checked", "false");
  });

  it("配置字段缺省回退内置 true/15", async () => {
    render(<RemoteDesktopCard model={idleModel()} />);
    await waitFor(() => expect(configGetMock).toHaveBeenCalled());
    const fpsInput = screen.getByLabelText("帧率（fps）") as HTMLInputElement;
    expect(fpsInput.value).toBe("15");
    expect(
      screen.getByRole("switch", { name: "新会话需审批" }),
    ).toHaveAttribute("aria-checked", "true");
  });
});