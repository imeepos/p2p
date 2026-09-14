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

const FRIENDS = [
  {
    peerId: "12D3KooWFriendAlice000001",
    nickname: "Alice",
    note: null,
    addrs: [],
  },
  {
    peerId: "12D3KooWFriendBob00000002",
    nickname: "Bob",
    note: null,
    addrs: [],
  },
];

vi.mock("@/lib/ipc", () => ({
  ipc: {
    tunnelStatus: () => statusMock(),
    tunnelOpenDsh: () =>
      Promise.reject(new Error("DSH 入口不应被通用表单调用")),
    tunnelOpen: (target?: string, peer?: string) => openMock(target, peer),
    onTunnelStatus: (h?: (s: unknown) => void) => onTunnelStatusMock(h),
  },
}));

// 好友选择器数据面：chat-store 好友列表（friendsLoaded=true 免预热装载）。
vi.mock("@/stores/chat-store", () => ({
  useChatStore: (selector?: (s: unknown) => unknown) => {
    const state = {
      friends: FRIENDS,
      friendsLoaded: true,
      friendsError: null,
    };
    return selector ? selector(state) : state;
  },
}));

// 被访服务卡的本机 PeerId 读取面：与通用卡同页挂载，需可独立渲染。
vi.mock("@/stores/node-store", () => ({
  useNodeStore: (selector?: (s: unknown) => unknown) => {
    const state = { status: { peerId: "SelfPeerId1234567890" } };
    return selector ? selector(state) : state;
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

  it("好友选择器：展开列出好友，选中后回填手填输入框", async () => {
    render(<RemoteAccessView />);
    fireEvent.click(await screen.findByTestId("tunnel-peer-picker"));
    const panel = await screen.findByTestId("tunnel-peer-picker-panel");
    expect(panel.textContent).toContain("Alice");
    expect(panel.textContent).toContain("Bob");
    fireEvent.click(screen.getByRole("option", { name: /Alice/ }));
    const manual = screen.getByLabelText(
      "通用被访节点 PeerId",
    ) as HTMLInputElement;
    expect(manual.value).toBe("12D3KooWFriendAlice000001");
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

  it("成功横幅：含入口链接与对端节点，可关闭", async () => {
    render(<RemoteAccessView />);
    fireEvent.change(await screen.findByLabelText("服务端口"), {
      target: { value: "3000" },
    });
    fireEvent.change(screen.getByLabelText("通用被访节点 PeerId"), {
      target: { value: "peerid" },
    });
    fireEvent.click(screen.getByRole("button", { name: "打开通用服务" }));
    const banner = await screen.findByTestId("tunnel-terminal-banner");
    expect(banner.textContent).toContain("http://127.0.0.1:40001");
    expect(banner.textContent).toContain("peerid");
    fireEvent.click(screen.getByRole("button", { name: "知道了" }));
    await waitFor(() =>
      expect(screen.queryByTestId("tunnel-terminal-banner")).toBeNull(),
    );
  });

  it("错误：服务端报错进入错误盒，横幅含原因与闭集错误码", async () => {
    openMock.mockImplementation(async () => {
      throw new Error("tunnel dial_failed: 对端拒绝");
    });
    render(<RemoteAccessView />);
    fireEvent.change(await screen.findByLabelText("服务端口"), {
      target: { value: "3000" },
    });
    fireEvent.change(screen.getByLabelText("通用被访节点 PeerId"), {
      target: { value: "peerid" },
    });
    fireEvent.click(screen.getByRole("button", { name: "打开通用服务" }));
    const banner = await screen.findByTestId("tunnel-terminal-banner");
    expect(banner.textContent).toContain("tunnel dial_failed: 对端拒绝");
    expect(banner.getAttribute("data-tone")).toBe("error");
    const code = screen.getByTestId("tunnel-banner-code");
    expect(code.textContent).toContain("dial_failed");
    expect(code.textContent).toContain("对端服务拨号失败");
  });
});
