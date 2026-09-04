import { beforeEach, describe, expect, it, vi } from "vitest";

const diagMocks = vi.hoisted(() => ({
  logPath: vi.fn(),
  logTail: vi.fn(),
  logClear: vi.fn(),
}));
const errMocks = vi.hoisted(() => ({
  getRecentErrors: vi.fn<
    () => Array<{ ts: string; kind: string; message: string; stack: string | null }>
  >(() => []),
  clearErrorBufferAndQueue: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({ ipc: {}, diag: diagMocks }));
vi.mock("@/lib/error-report", () => errMocks);

import { diagnosticsPage } from "./diagnostics-page";
import { executePageAction } from "../page-registry";

beforeEach(() => {
  vi.clearAllMocks();
  errMocks.getRecentErrors.mockReturnValue([]);
});

describe("diagnostics 页 descriptor", () => {
  it("descriptor 快照与动作清单", () => {
    expect(diagnosticsPage.descriptor).toMatchSnapshot();
    expect(diagnosticsPage.descriptor.actions.map((a) => a.name)).toEqual([
      "refresh",
      "clearAll",
    ]);
  });

  it("state 与错误缓冲卡同源（getRecentErrors 派生）", () => {
    errMocks.getRecentErrors.mockReturnValue([
      { ts: "t1", kind: "console", message: "boom", stack: null },
    ]);
    const snapshot = diagnosticsPage.state?.() as {
      recentErrors: Array<{ ts: string; kind: string; message: string }>;
    };
    expect(snapshot.recentErrors).toEqual([
      { ts: "t1", kind: "console", message: "boom" },
    ]);
  });

  it("clearAll 缺 confirm 结构化拒绝且不触达清空路径", async () => {
    await expect(executePageAction("diagnostics", "clearAll", {})).resolves.toMatchObject({
      ok: false,
      error: { code: "ACTION_CONFIRM_REQUIRED" },
    });
    expect(errMocks.clearErrorBufferAndQueue).not.toHaveBeenCalled();
    expect(diagMocks.logClear).not.toHaveBeenCalled();
  });

  it("refresh 与刷新按钮同源（logPath + logTail 并行取数）", async () => {
    diagMocks.logPath.mockResolvedValue("/tmp/frontend.log");
    diagMocks.logTail.mockResolvedValue(["line-1"]);
    const result = await executePageAction("diagnostics", "refresh", {});
    expect(result).toMatchObject({
      ok: true,
      data: { logPath: "/tmp/frontend.log", tail: ["line-1"] },
    });
    expect(diagMocks.logTail).toHaveBeenCalledWith(50);
  });

  it("refresh 支持自定义 tailLines", async () => {
    diagMocks.logTail.mockResolvedValue([]);
    await executePageAction("diagnostics", "refresh", { tailLines: 5 });
    expect(diagMocks.logTail).toHaveBeenCalledWith(5);
  });

  it("clearAll 带 confirm 按序清缓冲与日志文件", async () => {
    diagMocks.logClear.mockResolvedValue(undefined);
    const result = await executePageAction("diagnostics", "clearAll", { confirm: true });
    expect(result).toMatchObject({ ok: true, data: { cleared: true } });
    expect(errMocks.clearErrorBufferAndQueue).toHaveBeenCalled();
    expect(diagMocks.logClear).toHaveBeenCalled();
  });
});
