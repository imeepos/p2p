import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const statusMock = vi.fn(async () => idleStatus());
const openMock = vi.fn(async (_target?: string, _peer?: string) => ({
  localAddr: "127.0.0.1:40001",
  openUrl: "http://127.0.0.1:40001",
  token: "",
}));
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
    tunnelOpenDsh: () =>
      Promise.reject(new Error("DSH 入口不应被通用表单调用")),
    tunnelOpen: (target?: string, peer?: string) => openMock(target, peer),
    onTunnelStatus: (h?: (s: unknown) => void) => onTunnelStatusMock(h),
  },
}));

import type { TunnelStatusReport } from "@/lib/ipc-types";

import "@/i18n";
import { RemoteAccessView } from "./remote-access-view";

beforeEach(() => {
  statusMock.mockClear();
  openMock.mockClear();
  onTunnelStatusMock.mockClear();
  statusMock.mockImplementation(async () => idleStatus());
  openMock.mockImplementation(async () => ({
    localAddr: "127.0.0.1:40001",
    openUrl: "http://127.0.0.1:40001",
    token: "",
  }));
});

describe("RemoteAccessView 通用服务表单", () => {
  it("空态：渲染通用表单且输入为空时按钮禁用", async () => {
    render(<RemoteAccessView />);
    expect(await screen.findByText("通用服务")).toBeTruthy();
    expect(screen.getByLabelText("服务端口")).toBeTruthy();
    expect(screen.getByLabelText("通用被访节点 PeerId")).toBeTruthy();
    const button = screen.getByRole("button", { name: "打开通用服务" });
    expect((button as HTMLButtonElement).disabled).toBe(true);
  });

  it("成功：提交端口+peer 调 tunnel_open 并展示 local_addr 可复制链接", async () => {
    render(<RemoteAccessView />);
    fireEvent.change(await screen.findByLabelText("服务端口"), {
      target: { value: "3000" },
    });
    fireEvent.change(screen.getByLabelText("通用被访节点 PeerId"), {
      target: { value: "peerid" },
    });
    fireEvent.click(screen.getByRole("button", { name: "打开通用服务" }));
    await waitFor(() =>
      expect(screen.getAllByText("已开启").length).toBeGreaterThan(0),
    );
    expect(openMock).toHaveBeenCalledWith("127.0.0.1:3000", "peerid");
    expect(screen.getAllByText("127.0.0.1:40001").length).toBeGreaterThan(0);
    expect(
      screen.getAllByText("http://127.0.0.1:40001").length,
    ).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "复制链接" })).toBeTruthy();
  });

  it("错误：坏 target 服务端报错进入错误态并展示最近错误", async () => {
    openMock.mockImplementation(async () => {
      throw new Error("target 只允许 127.0.0.1 回环字面量: localhost");
    });
    render(<RemoteAccessView />);
    fireEvent.change(await screen.findByLabelText("服务端口"), {
      target: { value: "3000" },
    });
    fireEvent.change(screen.getByLabelText("通用被访节点 PeerId"), {
      target: { value: "peerid" },
    });
    fireEvent.click(screen.getByRole("button", { name: "打开通用服务" }));
    const alerts = await screen.findAllByRole("alert");
    expect(alerts.length).toBeGreaterThan(0);
    expect(document.body.textContent).toContain(
      "target 只允许 127.0.0.1 回环字面量",
    );
  });
});
