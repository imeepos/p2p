import { describe, expect, it } from "vitest";

import {
  acpCloseHelpKey,
  acpErrorDetail,
  acpErrorHelpKey,
  ACP_ERROR_HELP,
  connectFailureText,
} from "./error-help";
import { ACP_ERROR_KEYS } from "./store-events";

const t = (key: unknown): string => String(key);

describe("acp error-help 映射（F06）", () => {
  it("ACP_ERROR_KEYS 的每个错误码都有人话键，无码落兜底", () => {
    for (const code of Object.keys(ACP_ERROR_KEYS)) {
      expect(ACP_ERROR_HELP[code]).toBeTruthy();
      expect(acpErrorHelpKey(code)).toBe(ACP_ERROR_HELP[code]);
    }
    expect(acpErrorHelpKey("mysteryCode")).toBe("uxgAcp.fallback");
  });

  it("1006 空 reason 映射为检查 token；其余关闭帧按 kind 映射", () => {
    expect(acpCloseHelpKey({ kind: "abnormal", code: 1006, reason: "" })).toBe(
      "acp.reconnect.checkToken",
    );
    expect(
      acpCloseHelpKey({ kind: "dial-failed", code: 1007, reason: "x" }),
    ).toBe("acp.connection.closeDialFailed");
    expect(acpCloseHelpKey({ kind: "abnormal", code: 1011, reason: "boom" })).toBe(
      "acp.connection.closeAbnormal",
    );
  });

  it("详情串组合错误码、关闭码与端点；closed 帧不进详情", () => {
    expect(
      acpErrorDetail({
        lastError: "endpointIncomplete",
        closeInfo: { kind: "closed", code: 1000, reason: "" },
        wsUrl: "ws://x",
      }),
    ).toBe("error=endpointIncomplete ws=ws://x");
    expect(
      acpErrorDetail({
        lastError: null,
        closeInfo: { kind: "abnormal", code: 1006, reason: "" },
      }),
    ).toBe("close=abnormal(code=1006)");
    expect(acpErrorDetail({ lastError: null, closeInfo: null })).toBe("");
  });

  it("行内文案取序：lastError 优先，其次关闭帧，最后兜底", () => {
    expect(connectFailureText(t, "promptFailed", null)).toBe(
      "uxgAcp.promptFailed",
    );
    expect(
      connectFailureText(t, null, { kind: "denied", code: 4003, reason: "" }),
    ).toBe("acp.connection.closeDenied");
    expect(connectFailureText(t, null, null)).toBe("uxgAcp.fallback");
  });
});
