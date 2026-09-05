// 会话侧栏可访问性测试（P2 页面打磨）：活动会话行带 aria-current，
// 关闭 icon-only 按钮带 aria-label。
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { useAcpStore } from "@/acp/acp-store";
import { SessionSidebar } from "./session-sidebar";

await import("@/i18n");

beforeEach(() => {
  useAcpStore.getState().resetConsoleState();
  useAcpStore.setState({
    phase: "online",
    sessions: [
      { sessionId: "s-1", title: "first" },
      { sessionId: "s-2", title: "second" },
    ],
    activeSessionId: "s-2",
  });
});

describe("SessionSidebar 可访问性", () => {
  it("活动会话 aria-current=true，非活动会话无该属性", () => {
    render(<SessionSidebar />);
    const active = screen.getByTestId("acp-session-row-s-2").querySelector("button");
    const inactive = screen.getByTestId("acp-session-row-s-1").querySelector("button");
    expect(active?.getAttribute("aria-current")).toBe("true");
    expect(inactive?.getAttribute("aria-current")).toBeNull();
  });

  it("关闭按钮为 icon-only 但带 aria-label", () => {
    render(<SessionSidebar />);
    const close = screen.getByTestId("acp-session-close-s-1");
    expect(close.getAttribute("aria-label")).toBe("关闭会话");
  });

  it("离线时 resume/close 禁用，与新建按钮一致（P2-ADD 需求8）", () => {
    useAcpStore.setState({ phase: "idle" });
    render(<SessionSidebar />);
    const row = screen.getByTestId("acp-session-row-s-1");
    expect((row.querySelector("button") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId("acp-session-close-s-1") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId("acp-session-new") as HTMLButtonElement).disabled).toBe(true);
  });

  it("在线时 resume/close 可用", () => {
    render(<SessionSidebar />);
    const row = screen.getByTestId("acp-session-row-s-1");
    expect((row.querySelector("button") as HTMLButtonElement).disabled).toBe(false);
    expect((screen.getByTestId("acp-session-close-s-1") as HTMLButtonElement).disabled).toBe(false);
  });
});
