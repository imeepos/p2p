// 用量条阈值分级测试（P2 页面打磨）：80% 切警示色+文字提示，95% 升危急。
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { useAcpStore } from "@/acp/acp-store";
import { UsageBar } from "./usage-bar";

await import("@/i18n");

const SID = "s-usage";

function seed(used: number, size: number) {
  useAcpStore.setState((s) => ({
    activeSessionId: SID,
    interactions: {
      ...s.interactions,
      [SID]: { ...(s.interactions[SID] ?? { permissions: [], configOptions: [] }), usage: { used, size } },
    },
  }));
}

beforeEach(() => {
  useAcpStore.getState().resetConsoleState();
});

describe("UsageBar 阈值分级", () => {
  it("低于 80% 为常态：success 填充、无警示提示", () => {
    seed(53000, 200000);
    render(<UsageBar />);
    expect((screen.getByTestId("acp-usage-fill") as HTMLElement).className).toContain("bg-success");
    expect(screen.queryByTestId("acp-usage-hint")).toBeNull();
  });

  it("超过 80% 切警示色并显示文字提示，进度条带 role/aria-valuenow（P2-ADD 需求12）", () => {
    seed(170000, 200000);
    render(<UsageBar />);
    const fill = screen.getByTestId("acp-usage-fill");
    expect(fill.className).toContain("bg-warning");
    expect(fill.getAttribute("role")).toBe("progressbar");
    expect(fill.getAttribute("aria-valuenow")).toBe("85");
    expect(fill.getAttribute("aria-valuemin")).toBe("0");
    expect(fill.getAttribute("aria-valuemax")).toBe("100");
    expect(fill.getAttribute("aria-label")).toBe("上下文占用");
    const hint = screen.getByTestId("acp-usage-hint");
    expect(hint.textContent).toContain("80%");
    expect(hint.className).toContain("text-warning");
  });

  it("超过 95% 升级危急色与危急提示", () => {
    seed(194000, 200000);
    render(<UsageBar />);
    expect((screen.getByTestId("acp-usage-fill") as HTMLElement).className).toContain("bg-destructive");
    const hint = screen.getByTestId("acp-usage-hint");
    expect(hint.textContent).toContain("耗尽");
    expect(hint.className).toContain("text-destructive");
  });
});
