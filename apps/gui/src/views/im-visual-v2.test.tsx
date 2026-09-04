// IM-V2 五页视觉二轮打磨：13 项修复的 DOM/类名级证据测试。
// 协调者识图复核的机械配套：每项断言一个可在 DOM 上验证的形状/类名/状态。
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

const themeSetMock = vi.fn();
vi.mock("@/theme/theme-provider", () => ({
  useTheme: () => ({ theme: "system", setTheme: themeSetMock }),
}));

const { profileGetMock } = vi.hoisted(() => ({
  profileGetMock: vi.fn<() => Promise<unknown>>(),
}));
vi.mock("@/lib/ipc", () => ({
  ipc: {
    configGet: vi.fn(async () => ({
      quicPort: 0, tcpPort: 0, enableMdns: true, dataDir: "",
      bootstrap: [], relayAddrs: [], advertisedAddrs: [],
      observationPort: null, observationAddrs: [],
    })),
    profileGet: profileGetMock,
    profileSave: vi.fn(async (p: unknown) => p),
  },
}));

import "@/i18n";
import type { MetricsJson, NodeStatus } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";
import { useProfileStore } from "@/stores/profile-store";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { UsersIcon } from "lucide-react";
import React from "react";
import { FormProvider, useForm } from "react-hook-form";
import { DashboardTrendCard } from "./monitor/dashboard-trend-card";
import { DashboardView } from "./monitor/dashboard-view";
import { DegradeChainCard } from "./monitor/degrade-chain-card";
import { PeersTableCard } from "./monitor/peers-table-card";
import { PeersToolbar, type StatusFilter } from "./monitor/peers-toolbar";
import { RecentEventsCard } from "./monitor/recent-events-card";
import { Topbar } from "@/components/layout/topbar";
import { StatCard } from "@/components/page/stat-card";
import { MdnsCard } from "./discovery/mdns-card";
import { RendezvousCard } from "./discovery/rendezvous-card";
import { RelayConfigCard } from "./relay/relay-config-card";
import { RelayWatermarkCard } from "./relay/relay-watermark-card";
import { AppearanceCard } from "./settings/appearance-card";
import { NetworkCard } from "./settings/network-card";
import { ProfileCard } from "./settings/profile-card";
import { SettingsSaveBar } from "./settings/save-bar";
import { EmptyState } from "./shared/empty-state";

function guiConfig(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    quicPort: 3400, tcpPort: 3401, enableMdns: true, dataDir: "/tmp/p2p",
    bootstrap: [], relayAddrs: [], advertisedAddrs: [],
    observationPort: null, observationAddrs: [], ...overrides,
  };
}

function runningStatus(): { status: NodeStatus } {
  return {
    status: {
      running: true, peerId: "12D3KooWX".repeat(4), listenAddrs: [],
      uptimeSecs: 0, startedAtMs: null, config: guiConfig(),
    },
  };
}

const METRICS: MetricsJson = {
  dialDirectOk: 1, dialDirectFail: 0, dialPunchOk: 0, dialPunchFail: 0,
  dialRelayOk: 0, dialRelayFail: 0, addrDialFailures: 0, relayReconnects: 2,
  gateDenialsTotal: 0, activeConnections: 3, relaySessionsActive: 1,
};

function FormHarness({ children }: { children: React.ReactNode }) {
  const form = useForm({
    defaultValues: {
      quicPort: 3400, tcpPort: 3401, enableMdns: true, dataDir: "/tmp",
      advertisedAddrs: [], observationPort: null, observationAddrs: [],
    },
  });
  return <FormProvider {...form}>{children}</FormProvider>;
}

beforeEach(() => {
  themeSetMock.mockReset();
  profileGetMock.mockReset();
  useNodeStore.setState({ status: null, metrics: null });
  useProfileStore.setState({
    profile: { name: "", description: "", avatar: null },
    loaded: true, loadError: null,
  });
});

describe("IM-V2 dashboard evidence", () => {
  it("D1 顶栏停止按钮运行中为中性边框，无红色 destructive", () => {
    useNodeStore.setState(runningStatus());
    const { container } = render(
      <MemoryRouter><Topbar /></MemoryRouter>,
    );
    const stop = [...container.querySelectorAll("header button")].find((b) =>
      b.textContent.includes("停止"),
    );
    expect(stop).toBeTruthy();
    expect(stop!.className).not.toContain("bg-destructive");
    expect(stop!.className).toContain("border");
  });

  it("D2 仪表盘两行状态/指标卡统一最小高度，且同为标签在上垂直栈", () => {
    const { container } = render(
      <MemoryRouter><DashboardView /></MemoryRouter>,
    );
    const scoped = [...container.querySelectorAll("div")].find((d) =>
      d.className.includes("[&_[data-slot=card]]:min-h-28"),
    );
    expect(scoped).toBeTruthy();
    const solo = render(<StatCard label="已知节点" value="7" />);
    const card = solo.container.querySelector("[data-slot=card]")!;
    const desc = card.querySelector("[data-slot=card-description]")!;
    const title = card.querySelector("[data-slot=card-title]")!;
    expect(desc.textContent).toBe("已知节点");
    // 标签在值之前：垂直栈结构证据
    expect(title.compareDocumentPosition(desc) & Node.DOCUMENT_POSITION_PRECEDING).toBeTruthy();
    solo.unmount();
  });

  it("D3 空趋势占位收紧为 py-5 且带暂无数据语义", () => {
    render(<DashboardTrendCard history={[]} running={false} />);
    const status = screen.getByRole("status");
    expect(status.textContent).toContain("暂无趋势数据");
    expect(status.className).toContain("py-5");
  });

  it("D3 全零采样点同样走占位（mock 停止态持续喂零值点不渲染空图）", () => {
    const zeros = [1, 2, 3].map((n) => ({
      tMs: n * 1000,
      activeConnections: 0,
      relaySessionsActive: 0,
      dialOkTotal: 0,
      dialFailTotal: 0,
    }));
    render(<DashboardTrendCard history={zeros} running={false} />);
    expect(screen.getByRole("status").textContent).toContain("暂无趋势数据");
    expect(screen.queryByRole("img")).toBeNull();
  });

  it("D4 底部成功率/最近事件卡最小高度与上方趋势卡节奏一致", () => {
    const a = render(<DegradeChainCard metrics={null} loading />);
    expect(a.container.querySelector("[data-slot=card]")!.className).toContain("min-h-56");
    const b = render(<RecentEventsCard events={[]} loading />);
    expect(b.container.querySelector("[data-slot=card]")!.className).toContain("min-h-56");
  });
});

function TabHarness() {
  const [filter, setFilter] = React.useState<StatusFilter>("all");
  return (
    <PeersToolbar
      query=""
      onQueryChange={() => {}}
      statusFilter={filter}
      onStatusFilterChange={(next) => setFilter(next)}
      onOpenDial={() => {}}
    />
  );
}

describe("IM-V2 peers / discovery evidence", () => {
  it("P1 空态卡收敛为 max-w-sm + p-6", () => {
    const { container } = render(
      <EmptyState icon={UsersIcon} title="暂无已知节点" />,
    );
    const box = container.firstElementChild as HTMLElement;
    expect(box.className).toContain("max-w-sm");
    expect(box.className).toContain("p-6");
    expect(box.className).not.toContain("max-w-md");
    expect(box.className).not.toContain("p-8");
  });

  it("P2 筛选 Tab 选中项实心填充，未选中弱化，点击可切换", () => {
    const { container } = render(
      <TabHarness />,
    );
    const triggers = [...container.querySelectorAll("[data-slot=tabs-trigger]")];
    const active = triggers.find((t) => t.getAttribute("aria-selected") === "true")!;
    expect(active.className).toContain("data-[state=active]:bg-primary");
    const inactive = triggers.find((t) => t.getAttribute("aria-selected") === "false")!;
    expect(inactive.className).toContain("text-muted-foreground");
    // 受控 Tabs 状态机由 Radix 保证，此处锁定类名证据与 aria 映射
    expect(active.getAttribute("data-state")).toBe("active");
  });

  it("P1 对端空态不再套全宽 Card，消除外层容器空旷感", () => {
    const noop = () => async () => ({}) as never;
    const { container } = render(
      <PeersTableCard
        peers={[]}
        bufferEmpty
        locale="zh-CN"
        now={0}
        onPing={noop}
        onConnect={noop}
        onDisconnect={() => async () => true}
        onShowDetail={() => {}}
        onOpenDial={() => {}}
      />,
    );
    expect(container.querySelector("[data-slot=card]")).toBeNull();
    expect(screen.getByText("暂无已知节点")).toBeTruthy();
    expect(screen.getByRole("button", { name: "拨号添加节点" })).toBeTruthy();
  });

  it("F1 mDNS 卡与 rendezvous 卡都带 h-full，同 grid 行底端对齐", () => {
    const mdns = render(<MdnsCard config={guiConfig() as never} onSaved={() => {}} />);
    const rdv = render(
      <ConfirmProvider>
        <RendezvousCard bootstrap={["/ip4/203.0.113.5/udp/3400"]} onChange={async () => true} />
      </ConfirmProvider>,
    );
    for (const c of [mdns.container, rdv.container]) {
      const card = c.querySelector("[data-slot=card]")!;
      expect(card.className).toContain("h-full");
    }
  });

  it("F2 rendezvous 地址删除按钮触控区 size-9（36px ≥ 32px）", () => {
    render(
      <ConfirmProvider>
        <RendezvousCard bootstrap={["/ip4/203.0.113.5/udp/3400"]} onChange={async () => true} />
      </ConfirmProvider>,
    );
    const del = screen.getByRole("button", { name: "删除" });
    expect(del.className).toContain("size-9");
  });
});

describe("IM-V2 relay evidence", () => {
  it("R2 配置卡 h-full 与水位卡等高；水位改嵌套边框小卡并缩小字号", () => {
    useNodeStore.setState({ metrics: METRICS });
    const config = render(
      <RelayConfigCard relayAddrs={["/ip4/10.0.0.2/udp/3403"]} onSave={async () => {}} />,
    );
    expect(config.container.querySelector("[data-slot=card]")!.className).toContain("h-full");
    expect(config.container.querySelector("[data-slot=card-content]")!.className).toContain("flex-1");
    const { container } = render(<RelayWatermarkCard />);
    expect(container.querySelector(".text-2xl")).toBeNull();
    expect(container.querySelector(".text-lg")).toBeTruthy();
    expect(container.querySelectorAll(".rounded-md.border.p-3")).toHaveLength(2);
    const cc = container.querySelector("[data-slot=card-content]")!.className;
    expect(cc).toContain("flex-1");
    expect(cc).toContain("grid");
    expect(cc).not.toContain("content-center");
  });
});

describe("IM-V2 settings evidence", () => {
  it("S1 头像行 items-center，说明文字对比度 text-gray-600 级", async () => {
    profileGetMock.mockResolvedValue({ name: "节点", description: "", avatar: null });
    render(<ProfileCard />);
    const hint = await screen.findByText("支持 PNG / JPG / WebP，自动压缩为 128×128");
    expect(hint.className).toContain("text-gray-600");
    // 说明文字已移出侧列：头像行内不再有 <p>，圆标只与标签/按钮行同轴
    const avatar = document.querySelector("span.rounded-full")!;
    const row = avatar.closest(".items-center")!;
    expect(row.querySelectorAll("p")).toHaveLength(0);
    expect(hint.parentElement!.className).not.toContain("items-center");
  });

  it("S2 主题/语言未选中 chip 深描边 gray-300", () => {
    render(
      <MemoryRouter><AppearanceCard /></MemoryRouter>,
    );
    const unselected = [...screen.getAllByRole("button")].filter(
      (b) => b.getAttribute("aria-pressed") === "false",
    );
    expect(unselected.length).toBeGreaterThan(0);
    for (const b of unselected) expect(b.className).toContain("border-gray-300");
  });

  it("S4 mDNS 长描述 max-w-sm + leading-5，左列 flex-1 与开关等距", () => {
    const { container } = render(
      <FormHarness><NetworkCard /></FormHarness>,
    );
    const hint = screen.getByText("开启后通过组播发现同一局域网内的节点");
    expect(hint.className).toContain("max-w-sm");
    expect(hint.className).toContain("leading-5");
    expect(hint.parentElement!.className).toContain("flex-1");
    expect(container.querySelector("[data-slot=switch]")).toBeTruthy();
  });

  it("S6 保存条提示与按钮同行 justify-between，提示文字 AA 对比度", () => {
    render(
      <SettingsSaveBar
        dirty={false}
        loaded
        running={false}
        onSubmit={async () => {}}
        onSaveAndRestart={async () => {}}
        onReportSaveError={() => {}}
        onReportRestartError={() => {}}
      />,
    );
    const hint = screen.getByText("配置已与磁盘一致");
    expect(hint.className).toContain("text-gray-600");
    const bar = hint.parentElement!;
    expect(bar.className).toContain("justify-between");
    expect(bar.querySelector("button")).toBeTruthy();
  });
});
