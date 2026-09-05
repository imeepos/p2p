import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";

const configSaveMock = vi.fn(async (cfg: Record<string, unknown>) => cfg);
vi.mock("@/lib/ipc", () => ({
  ipc: {
    configGet: vi.fn(async () => ({
      quicPort: 3400, tcpPort: 3401, enableMdns: false, dataDir: "/tmp",
      bootstrap: [], relayAddrs: [], advertisedAddrs: [],
      observationPort: null, observationAddrs: [],
    })),
    configSave: (cfg: Record<string, unknown>) => configSaveMock(cfg),
  },
}));
vi.mock("@/lib/data-watch", () => ({
  markLocalWrite: vi.fn(),
  startDataWatch: vi.fn(async () => () => {}),
  registerReloader: vi.fn(() => () => {}),
}));

import "@/i18n";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import type { GuiConfig } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";
import { MdnsCard } from "./mdns-card";
import { DiscoveryView } from "./discovery-view";

function baseConfig(overrides: Partial<GuiConfig> = {}): GuiConfig {
  return {
    quicPort: 3400, tcpPort: 3401, enableMdns: false, dataDir: "/tmp",
    bootstrap: [], relayAddrs: [], advertisedAddrs: [],
    observationPort: null, observationAddrs: [], ...overrides,
  };
}

function renderCard(props: {
  config: GuiConfig;
  draft: boolean | null;
  running?: boolean;
}) {
  const onDraftChange = vi.fn();
  const onSave = vi.fn(async () => {});
  const onDiscard = vi.fn();
  render(
    <MdnsCard
      config={props.config}
      draft={props.draft}
      running={props.running ?? false}
      onDraftChange={onDraftChange}
      onSave={onSave}
      onDiscard={onDiscard}
    />,
  );
  return { onDraftChange, onSave, onDiscard };
}

beforeEach(() => {
  configSaveMock.mockClear();
  useNodeStore.setState({ status: null });
});

describe("MdnsCard 状态表述（按生效时机分态）", () => {
  it("关闭态：徽章「已关闭」，详情为关闭语义", () => {
    renderCard({ config: baseConfig(), draft: null });
    expect(screen.getByText("已关闭")).toBeTruthy();
    expect(screen.getByText(/已关闭局域网广播/)).toBeTruthy();
  });

  it("启用且节点未运行：徽章「已启用（下次启动生效）」，不进行时声称广播", () => {
    renderCard({ config: baseConfig({ enableMdns: true }), draft: null });
    expect(screen.getByText("已启用（下次启动生效）")).toBeTruthy();
    expect(screen.getByText(/下次启动后开始局域网广播/)).toBeTruthy();
    expect(screen.queryByText(/正在局域网内广播/)).toBeNull();
  });

  it("启用且节点运行中：详情明确重启后才开始广播、当前尚未广播", () => {
    useNodeStore.setState({ status: null });
    renderCard({
      config: baseConfig({ enableMdns: true }),
      draft: null,
      running: true,
    });
    expect(screen.getByText("已启用（下次启动生效）")).toBeTruthy();
    expect(screen.getByText(/重启节点后开始局域网广播/)).toBeTruthy();
    expect(screen.getByText(/尚未广播/)).toBeTruthy();
  });
});

describe("MdnsCard 置脏 + 保存条模型（与设置页一致）", () => {
  it("切换开关只上报草稿，不直接触发保存", () => {
    const { onDraftChange } = renderCard({ config: baseConfig(), draft: null });
    fireEvent.click(screen.getByRole("switch"));
    expect(onDraftChange).toHaveBeenCalledWith(true);
  });

  it("有草稿时展示未保存提示与保存/放弃按钮；展示值跟随草稿", () => {
    renderCard({ config: baseConfig(), draft: true });
    expect(screen.getByText("开关已修改，尚未保存")).toBeTruthy();
    expect(screen.getByRole("switch").getAttribute("aria-checked")).toBe("true");
    expect(screen.getByRole("button", { name: "保存" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "放弃" })).toBeTruthy();
  });

  it("点保存走 onSave，点放弃走 onDiscard", () => {
    const { onSave, onDiscard } = renderCard({ config: baseConfig(), draft: true });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    fireEvent.click(screen.getByRole("button", { name: "放弃" }));
    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onDiscard).toHaveBeenCalledTimes(1);
  });
});

describe("DiscoveryView mDNS 端到端（置脏到落盘）", () => {
  it("开关切换后点保存：configSave 收到新值，提示消失", async () => {
    render(
      <ConfirmProvider>
        <DiscoveryView />
      </ConfirmProvider>,
    );
    const sw = await screen.findByRole("switch");
    fireEvent.click(sw);
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => {
      expect(configSaveMock).toHaveBeenCalledTimes(1);
    });
    expect(configSaveMock.mock.calls[0][0]).toMatchObject({ enableMdns: true });
    await waitFor(() => {
      expect(screen.queryByText("开关已修改，尚未保存")).toBeNull();
    });
  });
});
