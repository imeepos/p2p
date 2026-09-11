import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const statusMock = vi.fn(async () => idleStatus());
const openMock = vi.fn(async (_url?: string, _peer?: string) => ({
  localAddr: "127.0.0.1:40001",
  openUrl: "http://127.0.0.1:40001/?token=tk",
  token: "tk",
}));
const onTunnelStatusMock = vi.fn(async (_h?: (s: unknown) => void) => () => {});

function idleStatus() {
  return {
    open: false,
    localAddr: null,
    openUrl: null,
    target: null,
    peer: null,
    activeConns: 0,
    lastError: null,
    visitedOpen: null,
    visitedAllowlist: null,
    visitedActiveSessions: null,
  };
}

vi.mock("@/lib/ipc", () => ({
  ipc: {
    tunnelStatus: () => statusMock(),
    tunnelOpenDsh: (url?: string, peer?: string) => openMock(url, peer),
    onTunnelStatus: (h?: (s: unknown) => void) => onTunnelStatusMock(h),
  },
}));

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

  it("开启成功转入已开启并展示本地地址与入口链接", async () => {
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
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("未指定被访节点 peer");
  });
});
