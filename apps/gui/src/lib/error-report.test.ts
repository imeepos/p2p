import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  clearRecentErrors,
  getRecentErrors,
  installErrorReport,
  readLocalLogLines,
} from "./error-report";

describe("error-report", () => {
  beforeEach(() => {
    localStorage.clear();
    clearRecentErrors();
    installErrorReport();
  });

  it("采集 window error 事件并降级写 localStorage", async () => {
    const before = getRecentErrors().length;
    window.dispatchEvent(
      new ErrorEvent("error", { message: "boom", error: new Error("boom") }),
    );
    const entries = getRecentErrors();
    const last = entries[entries.length - 1];
    expect(entries.length).toBe(before + 1);
    expect(last.kind).toBe("error");
    expect(last.message).toContain("boom");
    expect(last.stack).toContain("Error: boom");
    // 落盘经 400ms 防抖，等待刷写完成再断言。
    await vi.waitFor(() => {
      expect(readLocalLogLines().length).toBeGreaterThan(0);
    });
    const lines = readLocalLogLines();
    expect(JSON.parse(lines[lines.length - 1]!)).toMatchObject({ kind: "error" });
  });

  it("采集 unhandledrejection", () => {
    const event = new Event("unhandledrejection") as PromiseRejectionEvent;
    Object.defineProperty(event, "reason", { value: new Error("async boom") });
    window.dispatchEvent(event);
    const last = getRecentErrors()[getRecentErrors().length - 1];
    expect(last.kind).toBe("unhandledrejection");
    expect(last.message).toBe("async boom");
  });

  it("拦截 console.error 且原行为保留", () => {
    const before = getRecentErrors().length;
    console.error("[ErrorBoundary] 渲染异常：", new Error("render boom"));
    const last = getRecentErrors()[getRecentErrors().length - 1];
    expect(getRecentErrors().length).toBe(before + 1);
    expect(last?.kind).toBe("console");
    expect(last?.message).toContain("[ErrorBoundary]");
  });
});