import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { NodeStatus } from "@/lib/ipc-types";

const { peerDialMock } = vi.hoisted(() => ({ peerDialMock: vi.fn() }));

vi.mock("@/lib/ipc", () => ({ ipc: { peerDial: peerDialMock } }));

import "@/i18n";
import { useNodeStore } from "@/stores/node-store";
import { PeerDialDialog } from "./peer-dial-dialog";

const PEER_ID = "a".repeat(44);
const TARGET = `${PEER_ID}@192.168.1.9/u34001`;

const STATUS: NodeStatus = {
  running: false,
  peerId: null,
  listenAddrs: [],
  uptimeSecs: 0,
  startedAtMs: null,
  config: {
    quicPort: 0,
    tcpPort: 0,
    enableMdns: true,
    dataDir: "/tmp",
    bootstrap: [],
    relayAddrs: [],
    advertisedAddrs: [],
    observationPort: null,
    observationAddrs: [],
  },
};

function setStatus(running: boolean): void {
  useNodeStore.setState({ status: { ...STATUS, running } });
}

// 结构化三段填写（PeerId/地址/端口），传输默认 QUIC 不动
function fillSegments(peerId: string, addr: string, port: string): void {
  fireEvent.change(screen.getByLabelText("PeerId"), { target: { value: peerId } });
  fireEvent.change(screen.getByLabelText("地址"), { target: { value: addr } });
  fireEvent.change(screen.getByLabelText("端口"), { target: { value: port } });
}

describe("PeerDialDialog 结构化拨号", () => {
  beforeEach(() => {
    peerDialMock.mockReset();
    setStatus(false);
  });

  it("节点未运行时提交被拦截并给明确业务提示，不触发拨号", async () => {
    render(<PeerDialDialog open onOpenChange={vi.fn()} />);
    fillSegments(PEER_ID, "192.168.1.9", "34001");
    fireEvent.click(screen.getByRole("button", { name: "拨号" }));
    expect(
      await screen.findByText("节点未运行：请先启动节点，再手动拨号"),
    ).toBeInTheDocument();
    expect(peerDialMock).not.toHaveBeenCalled();
  });

  it("节点运行中正常提交拨号：三段组装为契约 §6 复合目标", async () => {
    setStatus(true);
    peerDialMock.mockResolvedValue({
      peer: PEER_ID,
      hops: [],
      ok: true,
      totalMs: 5,
    });
    render(<PeerDialDialog open onOpenChange={vi.fn()} />);
    fillSegments(PEER_ID, "192.168.1.9", "34001");
    fireEvent.click(screen.getByRole("button", { name: "拨号" }));
    await waitFor(() => expect(peerDialMock).toHaveBeenCalledTimes(1));
    expect(peerDialMock).toHaveBeenCalledWith(TARGET);
  });
});
