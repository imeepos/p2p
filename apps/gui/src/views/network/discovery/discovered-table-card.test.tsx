import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import "@/i18n";
import { useNodeStore, type PeerEntry } from "@/stores/node-store";
import { DiscoveredTableCard } from "./discovered-table-card";

const emptyPeerEntry: PeerEntry = {
  peerId: "peer-1",
  addrs: ["/ip4/10.0.0.2/udp/3400"],
  source: "mdns",
  connected: true,
  lastSeenMs: 1000,
  hops: [],
};

function renderCard(overrides: { mdnsActive?: boolean } = {}) {
  const onEnableMdns = vi.fn();
  const onAddAddress = vi.fn();
  render(
    <MemoryRouter>
      <DiscoveredTableCard
        mdnsActive={overrides.mdnsActive ?? false}
        onEnableMdns={onEnableMdns}
        onAddAddress={onAddAddress}
      />
    </MemoryRouter>,
  );
  return { onEnableMdns, onAddAddress };
}

beforeEach(() => {
  useNodeStore.setState({ peers: {}, events: [] });
});

describe("DiscoveredTableCard 空态入口（需求 8）", () => {
  it("空态文案下提供「开启 mDNS」与「添加引导地址」两个入口", () => {
    renderCard();
    expect(screen.getByText(/开启 mDNS 或添加引导地址/)).toBeTruthy();
    const enable = screen.getByRole("button", { name: "开启 mDNS" });
    const add = screen.getByRole("button", { name: "添加引导地址" });
    expect(enable).toBeTruthy();
    expect(add).toBeTruthy();
    fireEvent.click(enable);
    fireEvent.click(add);
  });

  it("mDNS 已启用时「开启 mDNS」入口禁用，添加入口仍可用", () => {
    renderCard({ mdnsActive: true });
    expect(
      (screen.getByRole("button", { name: "开启 mDNS" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(
      (screen.getByRole("button", { name: "添加引导地址" }) as HTMLButtonElement)
        .disabled,
    ).toBe(false);
  });

  it("非空态渲染结果表，不出现空态入口按钮", () => {
    useNodeStore.setState({ peers: { "peer-1": emptyPeerEntry }, events: [] });
    renderCard();
    expect(screen.getByText("peer-1")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "开启 mDNS" })).toBeNull();
    expect(screen.queryByRole("button", { name: "添加引导地址" })).toBeNull();
  });

  it("发现行补「拨号」「加为好友」快捷动作，目标串对齐契约 §6（F12）", () => {
    useNodeStore.setState({ peers: { "peer-1": emptyPeerEntry }, events: [] });
    renderCard();
    const dial = screen.getByTestId("discovery-dial-peer-1");
    expect(dial.getAttribute("href")).toBe(
      "/network/peers?dial=" +
        encodeURIComponent("peer-1@/ip4/10.0.0.2/udp/3400"),
    );
    const add = screen.getByTestId("discovery-add-friend-peer-1");
    expect(add.getAttribute("href")).toBe("/contacts?add=peer-1");
  });

  it("无地址的发现行拨号目标回退裸 PeerId", () => {
    useNodeStore.setState({
      peers: { "peer-1": { ...emptyPeerEntry, addrs: [] } },
      events: [],
    });
    renderCard();
    const dial = screen.getByTestId("discovery-dial-peer-1");
    expect(dial.getAttribute("href")).toBe(
      "/network/peers?dial=" + encodeURIComponent("peer-1"),
    );
  });
});
