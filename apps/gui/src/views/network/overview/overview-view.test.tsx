import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it } from "vitest";

import "@/i18n";
import type { MetricsJson, NodeEventJson, NodeStatus } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";
import { OverviewView } from "./overview-view";

// 4.2 概览页机械验收：四组信息卡 + 最近事件 5 条 + 查看全部 + 排障入口行。
// 复用原 dashboard 测试的 fixture 形状（node store 直设状态，不 mock IPC）。

const METRICS: MetricsJson = {
  dialDirectOk: 6, dialDirectFail: 2, dialPunchOk: 1, dialPunchFail: 1,
  dialRelayOk: 2, dialRelayFail: 0, addrDialFailures: 0, relayReconnects: 1,
  gateDenialsTotal: 4, activeConnections: 3, relaySessionsActive: 1,
};

const HISTORY = [
  { tMs: 1000, activeConnections: 1, relaySessionsActive: 0, dialOkTotal: 2, dialFailTotal: 0 },
  { tMs: 6000, activeConnections: 3, relaySessionsActive: 1, dialOkTotal: 7, dialFailTotal: 3 },
];

function guiConfig() {
  return {
    quicPort: 3400, tcpPort: 3401, enableMdns: true, dataDir: "/tmp/p2p",
    bootstrap: [], relayAddrs: [], advertisedAddrs: [],
    observationPort: null, observationAddrs: [],
  };
}

const STATUS: NodeStatus = {
  running: true,
  peerId: "12D3KooWXabcdefgh",
  listenAddrs: ["/ip4/127.0.0.1/udp/3400"],
  uptimeSecs: 0,
  startedAtMs: null,
  config: guiConfig(),
};

// 七条事件：只应渲染最新 5 条，第 6/7 条不得出现（4.2 第 4 块固定 5 条）。
function discoveredEvent(n: number): NodeEventJson {
  return {
    type: "peer_discovered",
    peer: "a".repeat(44),
    addrs: ["192.168.1." + n + "/u4000" + n],
    source: "mdns",
    tsMs: Date.now(),
  };
}

function summaryOf(n: number): string {
  return "发现节点 aaaaaaaa（192.168.1." + n + "/u4000" + n + "）";
}

beforeEach(() => {
  useNodeStore.setState({
    status: STATUS,
    metrics: METRICS,
    metricsHistory: HISTORY,
    events: [1, 2, 3, 4, 5, 6, 7].map(discoveredEvent),
    subscriptionLive: true,
    bootstrapPhase: "ready",
  });
});

function renderOverview() {
  // 查看全部与排障入口为 Link，需要路由上下文
  return render(
    <MemoryRouter>
      <OverviewView />
    </MemoryRouter>,
  );
}

describe("OverviewView 四组信息卡", () => {
  it("节点状态卡组：四张状态卡渲染且启停按钮常驻运行状态卡", () => {
    renderOverview();
    expect(screen.getByText("节点状态")).toBeInTheDocument();
    expect(screen.getByText("节点身份")).toBeInTheDocument();
    expect(screen.getByText("监听地址")).toBeInTheDocument();
    expect(screen.getByText("运行时长")).toBeInTheDocument();
    // 拍板项 5：启停就地可达；运行中启动禁用、停止可用
    const start = screen.getByRole("button", { name: "启动节点" });
    const stop = screen.getByRole("button", { name: "停止节点" });
    expect(start).toBeDisabled();
    expect(stop).toBeEnabled();
  });

  // 指标卡值断言走「卡片描述 -> 同卡值」结构定位：趋势卡系列标签与指标
  // 卡标签同名（中继会话/活跃连接），全局 getByText 会撞多重匹配。
  function cardValue(label: string): string {
    const desc = screen.getAllByText(label)[0]!;
    const card = desc.closest("[data-slot=card]")!;
    return card.querySelector("[data-slot=card-title]")?.textContent ?? "";
  }

  it("指标卡组：四指标渲染真实计数（数据源 metrics_get 口径不变）", () => {
    useNodeStore.setState({ peers: {} });
    renderOverview();
    expect(screen.getAllByText("已知节点").length).toBeGreaterThan(0);
    expect(screen.getAllByText("中继会话").length).toBeGreaterThan(0);
    expect(screen.getByText("门禁拒绝")).toBeInTheDocument();
    expect(cardValue("已知节点")).toBe("0");
    expect(cardValue("活跃连接")).toBe("3");
    expect(cardValue("中继会话")).toBe("1");
    expect(cardValue("门禁拒绝")).toBe("4");
  });

  it("趋势卡组：10 分钟窗口标题渲染", () => {
    renderOverview();
    expect(screen.getByText("10 分钟趋势")).toBeInTheDocument();
    expect(
      screen.getByText(/每 5 秒采样一个点，展示最近 120 点/),
    ).toBeInTheDocument();
  });

  it("拨号跳成功率卡：三行 direct/punch/relay 与成败计数", () => {
    renderOverview();
    expect(screen.getByText("拨号链成功率")).toBeInTheDocument();
    // 「中继」与排障入口链接同名，行存在性用多重匹配断言
    expect(screen.getAllByText("直连").length).toBeGreaterThan(0);
    expect(screen.getAllByText("打洞").length).toBeGreaterThan(0);
    expect(screen.getAllByText("中继").length).toBeGreaterThan(0);
    // 行尾成败计数为「成功 N / 失败 M」组合串（ChainBar 单文本节点）
    expect(screen.getByText("成功 6 / 失败 2")).toBeInTheDocument();
    expect(screen.getByText("成功 1 / 失败 1")).toBeInTheDocument();
    expect(screen.getByText("成功 2 / 失败 0")).toBeInTheDocument();
  });
});

describe("OverviewView 最近事件与排障入口", () => {
  it("最近事件固定 5 条：第 6 条起不得渲染", () => {
    renderOverview();
    for (let n = 1; n <= 5; n += 1) {
      expect(screen.getByText(summaryOf(n))).toBeInTheDocument();
    }
    expect(screen.queryByText(summaryOf(6))).toBeNull();
    expect(screen.queryByText(summaryOf(7))).toBeNull();
  });

  it("标题栏「查看全部」跳 /network/events", () => {
    renderOverview();
    expect(screen.getByRole("link", { name: "查看全部" })).toHaveAttribute(
      "href",
      "/network/events",
    );
  });

  it("排障入口行：节点/中继/诊断三链接直达对应 tab", () => {
    renderOverview();
    expect(screen.getByText("排障入口")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "节点" })).toHaveAttribute(
      "href",
      "/network/peers",
    );
    expect(screen.getByRole("link", { name: "中继" })).toHaveAttribute(
      "href",
      "/network/relay",
    );
    expect(screen.getByRole("link", { name: "诊断" })).toHaveAttribute(
      "href",
      "/network/diagnostics",
    );
  });
});
