import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const statusMock = vi.fn(async () => idleStatus());
const openMock = vi.fn(async (_url?: string, _peer?: string) => ({
  localAddr: "127.0.0.1:40001",
  openUrl: "http://127.0.0.1:40001/?token=tk",
  token: "tk",
}));

function statusWithSession() {
  const status = idleStatus();
  status.active = true;
  status.localAddr = "127.0.0.1:40001";
  status.target = "127.0.0.1:3080";
  status.sessions = [
    {
      sessionId: "0123456789abcdef",
      peerId: "peer",
      target: "127.0.0.1:3080",
      startedAt: 1000,
      endedAt: null,
      bytesIn: 128,
      bytesOut: 64,
      outcome: "open",
    },
  ];
  return status;
}

function statusWithRuntimeFailure() {
  const status = idleStatus();
  status.active = true;
  status.localAddr = "127.0.0.1:40001";
  status.target = "127.0.0.1:3080";
  status.sessions = [
    {
      sessionId: "fedcba9876543210",
      peerId: "peer",
      target: "127.0.0.1:3080",
      startedAt: 1000,
      endedAt: 2000,
      bytesIn: 0,
      bytesOut: 0,
      outcome: "busy",
    },
  ];
  return status;
}

const onTunnelStatusMock = vi.fn(async (_h?: (s: unknown) => void) => () => {});

function idleStatus(): TunnelStatusReport {
  return {
    active: false,
    localAddr: null,
    target: null,
    sessions: [],
    serve: { enabled: false, allow: [], activeSessions: 0 },
  };
}

vi.mock("@/lib/ipc", () => ({
  ipc: {
    tunnelStatus: () => statusMock(),
    tunnelOpenDsh: (url?: string, peer?: string) => openMock(url, peer),
    onTunnelStatus: (h?: (s: unknown) => void) => onTunnelStatusMock(h),
  },
}));

// 通用卡好友选择器与被访服务卡本机 PeerId 的读取面在本页同页挂载。
vi.mock("@/stores/chat-store", () => ({
  useChatStore: (selector?: (s: unknown) => unknown) => {
    const state = { friends: [], friendsLoaded: true, friendsError: null };
    return selector ? selector(state) : state;
  },
}));

vi.mock("@/stores/node-store", () => ({
  useNodeStore: (selector?: (s: unknown) => unknown) => {
    const state = { status: { peerId: "SelfPeerId1234567890" } };
    return selector ? selector(state) : state;
  },
}));

import type { TunnelStatusReport } from "@/lib/ipc-types";

import "@/i18n";
import { RemoteAccessView } from "./remote-access-view";

function renderView() {
  return render(<RemoteAccessView />);
}

beforeEach(() => {
  statusMock.mockClear();
  openMock.mockClear();
  onTunnelStatusMock.mockClear();
  statusMock.mockImplementation(async () => idleStatus());
  openMock.mockImplementation(async () => ({
    localAddr: "127.0.0.1:40001",
    openUrl: "http://127.0.0.1:40001/?token=tk",
    token: "tk",
  }));
});

describe("RemoteAccessView", () => {
  it("初始为未开启态并渲染表单", async () => {
    renderView();
    expect(await screen.findByText("未开启")).toBeTruthy();
    expect(screen.getByLabelText("DSH 启动 URL")).toBeTruthy();
    expect(screen.getByLabelText("被访节点 PeerId")).toBeTruthy();
  });

  it("开启成功转入已开启并展示本地地址、入口链接与 ws 派生地址", async () => {
    renderView();
    fireEvent.change(await screen.findByLabelText("DSH 启动 URL"), {
      target: { value: "http://127.0.0.1:3080/?token=tk" },
    });
    fireEvent.change(screen.getByLabelText("被访节点 PeerId"), {
      target: { value: "peerid" },
    });
    fireEvent.click(screen.getByRole("button", { name: "开启远程访问" }));
    await waitFor(() => screen.getByText("已开启"));
    expect(screen.getAllByText("127.0.0.1:40001").length).toBeGreaterThan(0);
    expect(screen.getAllByText(/40001\/\?token=tk/).length).toBeGreaterThan(0);
    // ws 提示与 http 链接并存（gap-matrix §3(a)：同端口，反代透传升级）。
    expect(
      screen.getAllByText("ws://127.0.0.1:40001").length,
    ).toBeGreaterThan(0);
    expect(screen.getByText(/ws 客户端可直连此地址/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "复制 WS 地址" })).toBeTruthy();
    expect(openMock).toHaveBeenCalledWith(
      "http://127.0.0.1:3080/?token=tk",
      "peerid",
    );
  });

  it("开启失败进入错误态并展示最近错误", async () => {
    openMock.mockImplementation(async () => {
      throw new Error("未指定被访节点 peer");
    });
    renderView();
    fireEvent.change(await screen.findByLabelText("DSH 启动 URL"), {
      target: { value: "http://127.0.0.1:3080/?token=tk" },
    });
    fireEvent.click(screen.getByRole("button", { name: "开启远程访问" }));
    await waitFor(() => screen.getByText("错误"));
    // 卡片错误盒与终态横幅同为 role=alert，断言任一可见即可。
    const alerts = await screen.findAllByRole("alert");
    expect(alerts.length).toBeGreaterThan(0);
    expect(
      alerts.some((alert) => alert.textContent?.includes("未指定被访节点 peer")),
    ).toBe(true);
  });

  it("成功横幅：含链接与对端节点，可关闭", async () => {
    renderView();
    fireEvent.change(await screen.findByLabelText("DSH 启动 URL"), {
      target: { value: "http://127.0.0.1:3080/?token=tk" },
    });
    fireEvent.change(screen.getByLabelText("被访节点 PeerId"), {
      target: { value: "peerid" },
    });
    fireEvent.click(screen.getByRole("button", { name: "开启远程访问" }));
    const banner = await screen.findByTestId("tunnel-terminal-banner");
    expect(banner.getAttribute("data-tone")).toBe("success");
    expect(banner.textContent).toContain("http://127.0.0.1:40001/?token=tk");
    expect(banner.textContent).toContain("peerid");
    fireEvent.click(screen.getByRole("button", { name: "知道了" }));
    await waitFor(() =>
      expect(screen.queryByTestId("tunnel-terminal-banner")).toBeNull(),
    );
  });

  it("运行中失败：事件推送闭集错误码终态记录后弹出失败横幅", async () => {
    let push: ((s: TunnelStatusReport) => void) | undefined;
    onTunnelStatusMock.mockImplementation(
      async (h?: (s: TunnelStatusReport) => void) => {
        push = h;
        return () => {};
      },
    );
    renderView();
    await screen.findByText("未开启");
    await push?.(statusWithRuntimeFailure());
    const banner = await screen.findByTestId("tunnel-terminal-banner");
    expect(banner.getAttribute("data-tone")).toBe("error");
    const code = screen.getByTestId("tunnel-banner-code");
    expect(code.textContent).toContain("busy");
    expect(code.textContent).toContain("对端并发已满");
  });

  it("事件推送 active 会话后展示审计行（sessionId/字节/终态）", async () => {
    let push: ((s: TunnelStatusReport) => void) | undefined;
    onTunnelStatusMock.mockImplementation(
      async (h?: (s: TunnelStatusReport) => void) => {
        push = h;
        return () => {};
      },
    );
    renderView();
    await screen.findByText("未开启");
    await push?.(statusWithSession());
    await waitFor(() =>
      expect(screen.getAllByText("已开启").length).toBeGreaterThan(0),
    );
    expect(screen.getByText("0123456789abcdef")).toBeTruthy();
    expect(screen.getByText("128/64 open")).toBeTruthy();
  });
});
