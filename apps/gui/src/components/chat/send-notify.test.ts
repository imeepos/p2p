// W1-17 回归防线：mark_failed 报告上浮 toast 的判定与触发——失败禁止零解释。
import { beforeEach, describe, expect, it, vi } from "vitest";

import { isFailedSendReport, notifyFailedSendReport } from "./send-notify";

vi.mock("@/components/feedback/toast", () => ({
  toastError: vi.fn(),
}));

import { toastError } from "@/components/feedback/toast";

const failedReport = (extra?: Partial<{ delivered: boolean; status: string }>) => ({
  delivered: extra?.delivered ?? false,
  message: { status: extra?.status ?? "failed", peer: "peerX", id: "m1" },
});

describe("send-notify mark_failed 判定", () => {
  beforeEach(() => vi.clearAllMocks());

  it("isFailedSendReport：delivered=false 且 status=failed 为真", () => {
    expect(isFailedSendReport(failedReport())).toBe(true);
  });

  it("isFailedSendReport：delivered=true 不算失败", () => {
    expect(isFailedSendReport(failedReport({ delivered: true }))).toBe(false);
  });

  it("isFailedSendReport：非 failed 状态不算失败", () => {
    expect(isFailedSendReport(failedReport({ status: "sent" }))).toBe(false);
  });

  it("notifyFailedSendReport：失败报告上浮 toast（不静默）", () => {
    notifyFailedSendReport(failedReport());
    expect(toastError).toHaveBeenCalledTimes(1);
  });

  it("notifyFailedSendReport：正常报告不打扰", () => {
    notifyFailedSendReport(failedReport({ delivered: true }));
    expect(toastError).not.toHaveBeenCalled();
  });
});
