import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it } from "vitest";

import "@/i18n";
import type { NodeStatus } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";
import { OverviewStatusCards } from "./overview-status-cards";

// F19 机械验收：停止后身份卡保留 PeerId（置灰 + 注明），不再落「未知」。

function guiConfig() {
  return {
    quicPort: 3400, tcpPort: 3401, enableMdns: true, dataDir: "/tmp/p2p",
    bootstrap: [], relayAddrs: [], advertisedAddrs: [],
    observationPort: null, observationAddrs: [],
  };
}

const PEER_ID = "12D3KooW" + "a".repeat(36);

function status(over: Partial<NodeStatus>): NodeStatus {
  return {
    running: true,
    peerId: PEER_ID,
    listenAddrs: ["/ip4/127.0.0.1/udp/3400"],
    uptimeSecs: 0,
    startedAtMs: null,
    config: guiConfig(),
    ...over,
  };
}

const RUNNING = status({});
const STOPPED_NO_ID = status({ running: false, peerId: null });

function renderCards(status: NodeStatus | null) {
  return render(
    <MemoryRouter>
      <OverviewStatusCards status={status} />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  useNodeStore.setState({ status: null });
});

describe("OverviewStatusCards 节点身份卡（F19）", () => {
  it("运行中显示 PeerId 缩略与复制，无保留注记", () => {
    renderCards(RUNNING);
    expect(screen.getByTitle(PEER_ID)).toBeInTheDocument();
    expect(screen.queryByText("已停止，身份保留")).toBeNull();
  });

  it("停止后身份保留：缩略置灰显示并注明「已停止，身份保留」", () => {
    const view = renderCards(RUNNING);
    view.rerender(
      <MemoryRouter>
        <OverviewStatusCards status={STOPPED_NO_ID} />
      </MemoryRouter>,
    );
    expect(screen.getByTitle(PEER_ID)).toBeInTheDocument();
    expect(screen.getByText("已停止，身份保留")).toBeInTheDocument();
    expect(screen.getByTestId("dashboard-identity")).toHaveClass("opacity-60");
  });

  it("从未持有身份时仍显示「未知」", () => {
    renderCards(STOPPED_NO_ID);
    expect(screen.getByText("未知")).toBeInTheDocument();
    expect(screen.queryByText("已停止，身份保留")).toBeNull();
  });
});
