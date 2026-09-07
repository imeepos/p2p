import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import "@/i18n";
import { useNodeStore } from "@/stores/node-store";
import { DataLinkBanner } from "./data-link-banner";

// R2-19 引导失败注入闭环：ipc 层注入失败/恢复，走真实 bootstrap→横幅→重试
// →恢复路径（非直改 store 状态），审计建议的 e2e 断言在组件层落地。
const { onNodeEventMock, nodeStatusMock, metricsGetMock } = vi.hoisted(() => ({
  onNodeEventMock: vi.fn(),
  nodeStatusMock: vi.fn(),
  metricsGetMock: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    onNodeEvent: onNodeEventMock,
    nodeStatus: nodeStatusMock,
    metricsGet: metricsGetMock,
  },
}));

function reset(
  over: Partial<{
    bootstrapPhase: "idle" | "loading" | "ready" | "error";
    bootstrapError: string | null;
    dataStale: boolean;
    lastRefreshError: string | null;
  }> = {},
): void {
  useNodeStore.setState({
    bootstrapPhase: "idle",
    bootstrapError: null,
    dataStale: false,
    lastRefreshError: null,
    subscriptionLive: false,
    ...over,
  });
}

describe("DataLinkBanner", () => {
  beforeEach(() => reset());

  it("引导失败给显式错误态与重试入口，而非永挂骨架", () => {
    reset({ bootstrapPhase: "error", bootstrapError: "sub boom" });
    render(<DataLinkBanner />);
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(
      screen.getByText("数据链路未就绪：事件订阅或初始刷新失败"),
    ).toBeInTheDocument();
    expect(screen.getByText("sub boom")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重试" })).toBeInTheDocument();
  });

  it("刷新连败显示数据可能已过期，恢复后横幅自动消失", () => {
    reset({ dataStale: true, lastRefreshError: "refresh boom" });
    const { container, rerender } = render(<DataLinkBanner />);
    expect(screen.getByRole("status")).toBeInTheDocument();
    expect(
      screen.getByText("数据可能已过期：状态刷新连续失败"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "立即刷新" }),
    ).toBeInTheDocument();

    // store 恢复（refresh 成功置 dataStale=false）→ 横幅派生消失。
    reset();
    rerender(<DataLinkBanner />);
    expect(screen.queryByRole("status")).toBeNull();
    expect(container).toBeEmptyDOMElement();
  });

  it("正常态不渲染横幅，不把最后一次数据当实时数据打扰用户", () => {
    reset({ bootstrapPhase: "ready" });
    const { container } = render(<DataLinkBanner />);
    expect(container).toBeEmptyDOMElement();
  });
});

// R2-19：引导失败注入 e2e——ipc 注入失败 → bootstrap 置 error → 横幅+重试；
// ipc 恢复后点「重试」走真实 bootstrap，横幅自动消失。
describe("DataLinkBanner 引导失败注入闭环（R2-19）", () => {
  beforeEach(() => {
    reset();
    onNodeEventMock.mockResolvedValue(() => {});
  });

  function stubStatus(mode: "fail" | "ok"): void {
    if (mode === "fail") {
      nodeStatusMock.mockRejectedValue(new Error("status boom"));
      metricsGetMock.mockRejectedValue(new Error("metrics boom"));
      return;
    }
    nodeStatusMock.mockResolvedValue({
      running: false,
      peerId: "self",
      listenAddrs: [],
      uptimeSecs: 0,
      startedAtMs: null,
      config: {},
    });
    metricsGetMock.mockResolvedValue({
      dialDirectOk: 0,
      dialDirectFail: 0,
      dialPunchOk: 0,
      dialPunchFail: 0,
      dialRelayOk: 0,
      dialRelayFail: 0,
      addrDialFailures: 0,
      relayReconnects: 0,
      gateDenialsTotal: 0,
      activeConnections: 0,
      relaySessionsActive: 0,
    });
  }

  it("注入引导失败出现横幅与重试按钮，恢复后点重试横幅消失", async () => {
    stubStatus("fail");
    const { container } = render(<DataLinkBanner />);
    expect(container).toBeEmptyDOMElement();

    await useNodeStore.getState().bootstrap();
    // 横幅出现：role=alert + 失败原因 + 重试按钮
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByText(/status boom/)).toBeInTheDocument();
    const retry = screen.getByRole("button", { name: "重试" });

    // ipc 恢复 → 点「重试」走真实 bootstrap → ready → 横幅自动消失
    stubStatus("ok");
    fireEvent.click(retry);
    await waitFor(() =>
      expect(useNodeStore.getState().bootstrapPhase).toBe("ready"),
    );
    await waitFor(() => expect(container).toBeEmptyDOMElement());
  });
});
