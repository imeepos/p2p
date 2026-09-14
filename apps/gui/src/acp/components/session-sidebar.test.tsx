// 会话侧栏渲染矩阵（uix-spec §2 两级树）：分组/折叠/当前组强制展开/行内限流/
// 搜索/选中态/空态/离线禁用/可访问性。
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { useAcpStore } from "@/acp/acp-store";
import type { SessionSummary } from "@/acp/protocol";
import { resetWorkspaceUiForTest } from "@/acp/workspace-ui-store";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { SessionSidebar } from "./session-sidebar";

await import("@/i18n");

// SessionRow 依赖全站确认弹框 context（关闭会话确认纪律），独立渲染需自持 Provider
function renderSidebar() {
  return render(
    <ConfirmProvider>
      <SessionSidebar />
    </ConfirmProvider>,
  );
}

function setSessions(sessions: SessionSummary[], activeSessionId: string | null = null) {
  useAcpStore.setState({ phase: "online", sessions, activeSessionId });
}

beforeEach(() => {
  useAcpStore.getState().resetConsoleState();
  resetWorkspaceUiForTest();
  setSessions([
    { sessionId: "s-1", title: "first" },
    { sessionId: "s-2", title: "second" },
  ], "s-2");
});

describe("SessionSidebar 两级树", () => {
  it("按 cwd 分组渲染组头，未分组组头恒排最后（I1）", () => {
    setSessions([
      { sessionId: "s-1", title: "a", cwd: "/tmp/alpha" },
      { sessionId: "s-2", title: "b" },
      { sessionId: "s-3", title: "c", cwd: "/tmp/beta" },
    ]);
    renderSidebar();
    expect(screen.getByTestId("acp-group-row-/tmp/alpha")).toBeTruthy();
    expect(screen.getByTestId("acp-group-row-/tmp/beta")).toBeTruthy();
    expect(screen.getByTestId("acp-group-row-ungrouped").textContent).toContain("未分组");
    const heads = screen.getAllByTestId(/^acp-group-row-/);
    expect(heads[heads.length - 1].dataset.testid).toBe("acp-group-row-ungrouped");
  });

  it("点组头折叠/展开：折叠后子行消失、aria-expanded=false（I3）", () => {
    setSessions([
      { sessionId: "s-1", title: "a", cwd: "/tmp/alpha" },
      { sessionId: "s-2", title: "b", cwd: "/tmp/beta" },
    ]);
    renderSidebar();
    expect(screen.getByTestId("acp-session-row-s-1")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-group-row-/tmp/alpha"));
    expect(screen.queryByTestId("acp-session-row-s-1")).toBeNull();
    expect(screen.getByTestId("acp-group-row-/tmp/alpha").getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(screen.getByTestId("acp-group-row-/tmp/alpha"));
    expect(screen.getByTestId("acp-session-row-s-1")).toBeTruthy();
  });

  it("含当前会话的组强制展开，折叠不生效（I3）", () => {
    setSessions([
      { sessionId: "s-1", title: "a", cwd: "/tmp/alpha" },
      { sessionId: "s-2", title: "b", cwd: "/tmp/beta" },
    ], "s-1");
    renderSidebar();
    fireEvent.click(screen.getByTestId("acp-group-row-/tmp/alpha"));
    expect(screen.getByTestId("acp-session-row-s-1")).toBeTruthy();
    // 对照组：不含当前会话的组可折叠
    fireEvent.click(screen.getByTestId("acp-group-row-/tmp/beta"));
    expect(screen.queryByTestId("acp-session-row-s-2")).toBeNull();
  });

  it("行内限流：7 条只显 5 行并显「展开 N 个」，点击临时全开（I4）", () => {
    setSessions(
      Array.from({ length: 7 }, (_, i) => ({ sessionId: "s-" + i, title: "t" + i, cwd: "/tmp/alpha" })),
    );
    renderSidebar();
    expect(screen.getByText("展开 2 个")).toBeTruthy();
    const visible = screen.getAllByTestId(/^acp-session-row-/);
    expect(visible).toHaveLength(5);
    fireEvent.click(screen.getByTestId("acp-group-expand-/tmp/alpha"));
    expect(screen.getAllByTestId(/^acp-session-row-/)).toHaveLength(7);
    expect(screen.queryByText("展开 2 个")).toBeNull();
  });

  it("搜索过滤会话，无结果显示空文案，Esc 清空还原树（I8）", () => {
    setSessions([
      { sessionId: "s-1", title: "alpha one", cwd: "/tmp/alpha" },
      { sessionId: "s-2", title: "beta two", cwd: "/tmp/beta" },
    ]);
    renderSidebar();
    const input = screen.getByTestId("acp-session-search") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "alpha" } });
    expect(screen.getByTestId("acp-session-row-s-1")).toBeTruthy();
    expect(screen.queryByTestId("acp-session-row-s-2")).toBeNull();
    fireEvent.change(input, { target: { value: "nomatch" } });
    expect(screen.getByText("无匹配会话")).toBeTruthy();
    fireEvent.keyDown(input, { key: "Escape" });
    expect(input.value).toBe("");
    expect(screen.getByTestId("acp-session-row-s-1")).toBeTruthy();
  });

  it("无会话显空态文案而非白板（I10）", () => {
    setSessions([]);
    renderSidebar();
    expect(screen.getByText("暂无会话")).toBeTruthy();
  });
});

describe("SessionSidebar 可访问性与离线禁用", () => {
  it("活动会话 aria-current=true，非活动会话无该属性", () => {
    renderSidebar();
    const active = screen.getByTestId("acp-session-row-s-2").querySelector("button");
    const inactive = screen.getByTestId("acp-session-row-s-1").querySelector("button");
    expect(active?.getAttribute("aria-current")).toBe("true");
    expect(inactive?.getAttribute("aria-current")).toBeNull();
  });

  it("关闭按钮为 icon-only 但带 aria-label", () => {
    renderSidebar();
    const close = screen.getByTestId("acp-session-close-s-1");
    expect(close.getAttribute("aria-label")).toBe("关闭会话");
  });

  it("离线时 resume/close/新建禁用，组头开合不受限", () => {
    useAcpStore.setState({ phase: "idle", activeSessionId: null });
    renderSidebar();
    const row = screen.getByTestId("acp-session-row-s-1");
    expect((row.querySelector("button") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId("acp-session-close-s-1") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId("acp-session-new") as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId("acp-group-row-ungrouped"));
    expect(screen.queryByTestId("acp-session-row-s-1")).toBeNull();
  });
});
