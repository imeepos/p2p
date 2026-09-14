import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const startMock = vi.fn(async (_target?: string) => ({
  enabled: true,
  allow: ["127.0.0.1:3080"],
  activeSessions: 0,
}));
const stopMock = vi.fn(async () => ({
  enabled: false,
  allow: ["127.0.0.1:3080"],
  activeSessions: 0,
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    tunnelServeStart: (target?: string) => startMock(target),
    tunnelServeStop: () => stopMock(),
  },
}));

// 本机 PeerId 来自 node-store 快照（对端说明中的 CLI 命令原文需要它）。
vi.mock("@/stores/node-store", () => ({
  useNodeStore: (selector?: (s: unknown) => unknown) => {
    const state = { status: { peerId: "SelfPeerId1234567890" } };
    return selector ? selector(state) : state;
  },
}));

import "@/i18n";
import type { TunnelServeStatus } from "@/lib/ipc-types";
import { TunnelServeCard } from "./tunnel-serve-card";

function idleServe(): TunnelServeStatus {
  return { enabled: false, allow: [], activeSessions: 0 };
}

beforeEach(() => {
  startMock.mockClear();
  stopMock.mockClear();
  startMock.mockImplementation(async (target?: string) => ({
    enabled: true,
    allow: [target ?? "127.0.0.1:3080"],
    activeSessions: 0,
  }));
  stopMock.mockImplementation(async () => ({
    enabled: false,
    allow: ["127.0.0.1:3080"],
    activeSessions: 0,
  }));
});

function renderCard(serve: TunnelServeStatus = idleServe()) {
  const onServeUpdate = vi.fn();
  const onError = vi.fn();
  const onBanner = vi.fn();
  render(
    <TunnelServeCard
      serve={serve}
      onServeUpdate={onServeUpdate}
      onError={onError}
      onBanner={onBanner}
    />,
  );
  return { onServeUpdate, onError, onBanner };
}

describe("隧道被访服务卡（gui-contract §19.1）", () => {
  it("空态：端口为空禁用开启，未受理时禁用关闭", () => {
    const { onError } = renderCard();
    const start = screen.getByTestId("tunnel-serve-start") as HTMLButtonElement;
    const stop = screen.getByTestId("tunnel-serve-stop") as HTMLButtonElement;
    expect(start.disabled).toBe(true);
    expect(stop.disabled).toBe(true);
    expect(screen.getByTestId("tunnel-serve-state").textContent).toBe("未受理");
    expect(screen.getByText("白名单为空")).toBeTruthy();
    expect(onError).not.toHaveBeenCalled();
  });

  it("开启：拼 127.0.0.1:<port> 字面量调用 start 并上抛报告", async () => {
    const { onServeUpdate } = renderCard();
    fireEvent.change(screen.getByLabelText("目标端口"), {
      target: { value: "3080" },
    });
    fireEvent.click(screen.getByTestId("tunnel-serve-start"));
    await waitFor(() =>
      expect(startMock).toHaveBeenCalledWith("127.0.0.1:3080"),
    );
    await waitFor(() =>
      expect(onServeUpdate).toHaveBeenCalledWith({
        enabled: true,
        allow: ["127.0.0.1:3080"],
        activeSessions: 0,
      }),
    );
  });

  it("白名单表格化：渲染端口/加入时间/状态表头与数据行", () => {
    renderCard({
      enabled: true,
      allow: ["127.0.0.1:3080", "127.0.0.1:9090"],
      activeSessions: 0,
    });
    expect(screen.getByTestId("tunnel-allow-table")).toBeTruthy();
    expect(screen.getByText("端口")).toBeTruthy();
    expect(screen.getByText("加入时间")).toBeTruthy();
    expect(screen.getByText("状态")).toBeTruthy();
    const rows = screen.getAllByTestId("tunnel-allow-row");
    expect(rows).toHaveLength(2);
    expect(rows[0]!.textContent).toContain("127.0.0.1:3080");
    expect(rows[0]!.textContent).toContain("受理中");
    expect(rows[0]!.textContent).toContain("—");
  });

  it("开启成功：生成对端说明（GUI 路径 + CLI 命令原文）并出现复制按钮", async () => {
    renderCard();
    fireEvent.change(screen.getByLabelText("目标端口"), {
      target: { value: "3080" },
    });
    fireEvent.click(screen.getByTestId("tunnel-serve-start"));
    const guide = await screen.findByTestId("tunnel-share-guide");
    expect(guide.textContent).toContain("3080");
    expect(guide.textContent).toContain("SelfPeerId1234567890");
    const cli = screen.getByTestId("tunnel-share-guide-cli");
    expect(cli.textContent).toContain(
      "p2pctl tunnel connect --peer SelfPeerId1234567890 --target 127.0.0.1:3080",
    );
    expect(screen.getByTestId("tunnel-share-guide-copy")).toBeTruthy();
  });

  it("开启成功：横幅上抛成功终态（含分享目标）", async () => {
    const { onBanner } = renderCard();
    fireEvent.change(screen.getByLabelText("目标端口"), {
      target: { value: "3080" },
    });
    fireEvent.click(screen.getByTestId("tunnel-serve-start"));
    await waitFor(() =>
      expect(onBanner).toHaveBeenCalledWith({
        tone: "success",
        target: "127.0.0.1:3080",
      }),
    );
  });

  it("开启失败：横幅上抛失败终态（原因与闭集错误码）", async () => {
    startMock.mockImplementation(async () => {
      throw new Error("tunnel dial_failed: 拨号被拒");
    });
    const { onServeUpdate, onBanner } = renderCard();
    fireEvent.change(screen.getByLabelText("目标端口"), {
      target: { value: "3080" },
    });
    fireEvent.click(screen.getByTestId("tunnel-serve-start"));
    await waitFor(() =>
      expect(onBanner).toHaveBeenCalledWith({
        tone: "error",
        reason: "tunnel dial_failed: 拨号被拒",
        code: "dial_failed",
      }),
    );
    expect(onServeUpdate).not.toHaveBeenCalled();
  });

  it("关闭：调 stop、白名单保留并上抛报告", async () => {
    const { onServeUpdate } = renderCard({
      enabled: true,
      allow: ["127.0.0.1:3080", "127.0.0.1:9090"],
      activeSessions: 2,
    });
    expect(screen.getByTestId("tunnel-serve-state").textContent).toBe(
      "受理中",
    );
    fireEvent.click(screen.getByTestId("tunnel-serve-stop"));
    await waitFor(() => expect(stopMock).toHaveBeenCalled());
    await waitFor(() =>
      expect(onServeUpdate).toHaveBeenCalledWith({
        enabled: false,
        allow: ["127.0.0.1:3080"],
        activeSessions: 0,
      }),
    );
    expect(screen.getByText(/127\.0\.0\.1:3080/)).toBeTruthy();
  });

  it("非法端口：服务端可读中文报错进错误盒并回调 onError", async () => {
    startMock.mockImplementation(async () => {
      throw new Error("目标须为 127.0.0.1:<port> 字面量（端口 1-65535），当前: 127.0.0.1:abc");
    });
    const { onServeUpdate } = renderCard();
    fireEvent.change(screen.getByLabelText("目标端口"), {
      target: { value: "abc" },
    });
    fireEvent.click(screen.getByTestId("tunnel-serve-start"));
    await waitFor(() =>
      expect(
        screen.getByTestId("tunnel-serve-error")?.textContent,
      ).toContain("127.0.0.1:abc"),
    );
    expect(onServeUpdate).not.toHaveBeenCalled();
    expect(screen.getByTestId("tunnel-serve-state").textContent).toBe(
      "未受理",
    );
  });
});
