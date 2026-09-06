import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";

import { CommandPalette } from "@/components/command-palette/command-palette";
import { useNodeStore } from "@/stores/node-store";
import type { NodeStatus } from "@/lib/ipc-types";
import { ThemeProvider } from "@/theme/theme-provider";
import "@/i18n";
import { Topbar } from "./topbar";

// cmdk 依赖 ResizeObserver 测量与 scrollIntoView 滚动，jsdom 均未实现：
// 仅在测试内补最小桩，不改变被测行为
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
(window as unknown as { ResizeObserver: unknown }).ResizeObserver = ResizeObserverStub;
if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {};
}



function stubPlatform(value: string) {
  Object.defineProperty(navigator, "platform", { configurable: true, value });
}

function renderTopbar() {
  return render(
    <ThemeProvider>
      <MemoryRouter>
        <Topbar />
      </MemoryRouter>
    </ThemeProvider>,
  );
}

// 与 app-layout 等价的受控壳：验证顶栏入口能真实打开面板
function Shell() {
  const [open, setOpen] = useState(false);
  return (
    <ThemeProvider>
      <MemoryRouter>
        <Topbar />
        <CommandPalette open={open} onOpenChange={setOpen} />
      </MemoryRouter>
    </ThemeProvider>
  );
}

afterEach(() => {
  cleanup();
  stubPlatform("");
});

describe("Topbar 命令面板入口", () => {
  it("渲染命令按钮；非 Apple 平台徽标为 Ctrl+K", () => {
    stubPlatform("Win32");
    renderTopbar();
    const button = screen.getByRole("button", { name: "命令面板" });
    expect(button.textContent).toContain("Ctrl+K");
  });

  it("Apple 平台徽标为 Cmd 系（\u2318K）", () => {
    stubPlatform("MacIntel");
    renderTopbar();
    expect(screen.getByRole("button", { name: "命令面板" }).textContent).toContain(
      "\u2318K",
    );
  });

  it("点击命令按钮打开命令面板", async () => {
    render(<Shell />);
    fireEvent.click(screen.getByRole("button", { name: "命令面板" }));
    expect(await screen.findByRole("dialog")).toBeTruthy();
  });
});

// F04：顶栏停止与概览状态卡同走 StopNodeDialog 二次确认，点击不再直接停机
function runningStatus(): NodeStatus {
  return {
    running: true,
    peerId: "PEER123",
    listenAddrs: [],
    uptimeSecs: 1,
    startedAtMs: null,
    config: {
      quicPort: 3400, tcpPort: 3401, enableMdns: true, dataDir: "/tmp",
      bootstrap: [], relayAddrs: [], advertisedAddrs: [],
      observationPort: null, observationAddrs: [],
    },
  };
}

describe("Topbar 停止节点二次确认（F04）", () => {
  afterEach(() => {
    useNodeStore.setState({ status: null });
  });

  it("运行中点击停止节点：先弹确认弹窗，未直接调用停止", () => {
    const stopNode = vi.fn(async () => runningStatus());
    useNodeStore.setState({ status: runningStatus(), stopNode });
    renderTopbar();
    fireEvent.click(screen.getByTestId("topbar-stop-node"));
    expect(screen.getByText("停止节点？")).toBeTruthy();
    expect(stopNode).not.toHaveBeenCalled();
  });

  it("确认弹窗取消：不停止，弹窗关闭", async () => {
    const stopNode = vi.fn(async () => runningStatus());
    useNodeStore.setState({ status: runningStatus(), stopNode });
    renderTopbar();
    fireEvent.click(screen.getByTestId("topbar-stop-node"));
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    await waitFor(() =>
      expect(screen.queryByText("停止节点？")).toBeNull(),
    );
    expect(stopNode).not.toHaveBeenCalled();
  });

  it("确认弹窗确认：调用停止", async () => {
    const stopNode = vi.fn(async () => {
      const stopped = { ...runningStatus(), running: false };
      useNodeStore.setState({ status: stopped });
      return stopped;
    });
    useNodeStore.setState({ status: runningStatus(), stopNode });
    renderTopbar();
    fireEvent.click(screen.getByTestId("topbar-stop-node"));
    fireEvent.click(screen.getByRole("button", { name: "确认" }));
    await waitFor(() => expect(stopNode).toHaveBeenCalled());
  });
});
