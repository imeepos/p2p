import type { FieldErrors } from "react-hook-form";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { SettingsFormValues } from "./config-schema";
import { focusFirstInvalidField } from "./focus-first-error";

type Errors = FieldErrors<SettingsFormValues>;

// P0 回归：FIELD_ORDER 缺 remote-access 三字段时，仅 rdFps 非法会得到
// invalidCount=0 → 跨节「保存」零反馈静默失败。本组用例锁死计数口径。
describe("focusFirstInvalidField 错误计数", () => {
  afterEach(() => vi.restoreAllMocks());

  it("无错误返回 0", () => {
    expect(focusFirstInvalidField({} as Errors)).toBe(0);
  });

  it("仅 rdFps 非法计 1（修复前为 0）", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(focusFirstInvalidField({ rdFps: { message: "x" } } as Errors)).toBe(1);
    expect(spy).toHaveBeenCalled();
  });

  it("tunnelServeAllow 与 rdRequireApproval 均计入", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const errors = {
      rdRequireApproval: { message: "x" },
      tunnelServeAllow: { message: "y" },
    } as unknown as Errors;
    expect(focusFirstInvalidField(errors)).toBe(2);
  });

  it("跨组错误按 FIELD_ORDER 取第一个（端口优先于 ftp）", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const errors = {
      ftpAccounts: { message: "a" },
      quicPort: { message: "b" },
    } as unknown as Errors;
    expect(focusFirstInvalidField(errors)).toBe(2);
  });
});
