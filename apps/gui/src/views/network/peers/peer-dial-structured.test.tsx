import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const { peerDialMock } = vi.hoisted(() => ({ peerDialMock: vi.fn() }));

vi.mock("@/lib/ipc", () => ({ ipc: { peerDial: peerDialMock } }));

import "@/i18n";
import { useNodeStore } from "@/stores/node-store";
import { PeerDialDialog } from "./peer-dial-dialog";

const PEER_ID = "a".repeat(44);

function seedDiscovered(): void {
  useNodeStore.setState({
    peers: {
      [PEER_ID]: {
        peerId: PEER_ID,
        addrs: ["192.168.1.9/u34001"],
        source: "mdns",
        connected: false,
        lastSeenMs: 1,
        hops: [],
      },
    },
  });
}

beforeEach(() => {
  peerDialMock.mockReset();
  useNodeStore.setState({ peers: {} });
});

describe("拨号目标结构化（F11）与端口即时校验（F14）", () => {
  it("端口失焦即时校验：非法端口就地提示且提交禁用，修正后恢复", async () => {
    // 节点须运行中，否则提交因「未运行」整体禁用，屏蔽端口校验的可恢复断言
    useNodeStore.setState({ status: { running: true } as never });
    render(<PeerDialDialog open onOpenChange={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("PeerId"), { target: { value: PEER_ID } });
    fireEvent.change(screen.getByLabelText("地址"), { target: { value: "192.168.1.9" } });
    const port = screen.getByTestId("dial-port");
    fireEvent.change(port, { target: { value: "99999" } });
    // 失焦前不提示（输入中不打断）；失焦口径同 settings-port-blur：
    // 真实交互同款 focus-then-blur 事件对（headless 程序化 blur 无事件，UX-J）
    expect(screen.queryByTestId("dial-port-error")).toBeNull();
    await act(async () => {
      port.focus();
      port.blur();
    });
    const error = screen.getByTestId("dial-port-error");
    expect(error.getAttribute("role")).toBe("alert");
    expect(port.getAttribute("aria-invalid")).toBe("true");
    expect(port.getAttribute("aria-describedby")).toBe("dial-port-error");
    expect(screen.getByRole("button", { name: "拨号" }).hasAttribute("disabled")).toBe(true);
    fireEvent.change(port, { target: { value: "34001" } });
    expect(screen.queryByTestId("dial-port-error")).toBeNull();
    expect(screen.getByRole("button", { name: "拨号" }).hasAttribute("disabled")).toBe(false);
  });

  it("从发现结果带入：选中候选连带预填主机/端口/传输三段", () => {
    seedDiscovered();
    render(<PeerDialDialog open onOpenChange={vi.fn()} />);
    fireEvent.click(screen.getByTestId("dial-peer-picker"));
    fireEvent.click(screen.getByRole("option"));
    expect((screen.getByLabelText("PeerId") as HTMLInputElement).value).toBe(PEER_ID);
    expect((screen.getByLabelText("地址") as HTMLInputElement).value).toBe("192.168.1.9");
    expect((screen.getByTestId("dial-port") as HTMLInputElement).value).toBe("34001");
    expect(screen.getByTestId("dial-transport").textContent).toBe("QUIC");
  });

  it("URL 契约：initialTarget 复合目标拆解预填三段；不可解析原串落 PeerId 段", () => {
    render(
      <PeerDialDialog
        open
        onOpenChange={vi.fn()}
        initialTarget={`${PEER_ID}@192.168.1.9/u34001`}
      />,
    );
    expect((screen.getByLabelText("PeerId") as HTMLInputElement).value).toBe(PEER_ID);
    expect((screen.getByLabelText("地址") as HTMLInputElement).value).toBe("192.168.1.9");
    expect((screen.getByTestId("dial-port") as HTMLInputElement).value).toBe("34001");
  });

  it("不可解析 initialTarget 原串保留在 PeerId 段，不静默丢弃", () => {
    render(<PeerDialDialog open onOpenChange={vi.fn()} initialTarget="not-a-target" />);
    expect((screen.getByLabelText("PeerId") as HTMLInputElement).value).toBe("not-a-target");
  });

  it("三段缺一不可提交；组合非法（PeerId 非法字符）提交时内联语法错误", async () => {
    const logSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    useNodeStore.setState({ status: { running: true } as never });
    render(<PeerDialDialog open onOpenChange={vi.fn()} />);
    expect(screen.getByRole("button", { name: "拨号" }).hasAttribute("disabled")).toBe(true);
    fireEvent.change(screen.getByLabelText("PeerId"), { target: { value: "!!short" } });
    fireEvent.change(screen.getByLabelText("地址"), { target: { value: "192.168.1.9" } });
    fireEvent.change(screen.getByTestId("dial-port"), { target: { value: "34001" } });
    fireEvent.click(screen.getByRole("button", { name: "拨号" }));
    expect(await screen.findByTestId("dial-syntax-error")).toBeTruthy();
    expect(peerDialMock).not.toHaveBeenCalled();
    logSpy.mockRestore();
  });
});
