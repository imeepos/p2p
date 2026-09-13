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
  render(
    <TunnelServeCard
      serve={serve}
      onServeUpdate={onServeUpdate}
      onError={onError}
    />,
  );
  return { onServeUpdate, onError };
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
