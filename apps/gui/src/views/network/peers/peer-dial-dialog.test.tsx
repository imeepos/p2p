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

function fillAndSubmit(): void {
  fireEvent.change(screen.getByLabelText("目标"), {
    target: { value: TARGET },
  });
  fireEvent.click(screen.getByRole("button", { name: "拨号" }));
}

describe("PeerDialDialog 节点运行校验", () => {
  beforeEach(() => {
    peerDialMock.mockReset();
    setStatus(false);
  });

  it("节点未运行时提交被拦截并给明确业务提示，不触发拨号", async () => {
    render(<PeerDialDialog open onOpenChange={vi.fn()} />);
    fillAndSubmit();
    expect(
      await screen.findByText("节点未运行：请先启动节点，再手动拨号"),
    ).toBeInTheDocument();
    expect(peerDialMock).not.toHaveBeenCalled();
  });

  it("节点运行中正常提交拨号", async () => {
    setStatus(true);
    peerDialMock.mockResolvedValue({
      peer: PEER_ID,
      hops: [],
      ok: true,
      totalMs: 5,
    });
    render(<PeerDialDialog open onOpenChange={vi.fn()} />);
    fillAndSubmit();
    await waitFor(() => expect(peerDialMock).toHaveBeenCalledTimes(1));
    expect(peerDialMock).toHaveBeenCalledWith(TARGET);
  });
});
