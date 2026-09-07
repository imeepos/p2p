import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import "@/i18n";
import { useUiPrefsStore } from "@/stores/ui-prefs-store";
import { InactiveGroupsToggle } from "./inactive-groups-toggle";

beforeEach(() => {
  localStorage.clear();
  useUiPrefsStore.setState({ showInactiveGroups: false });
});

describe("InactiveGroupsToggle（已退群显隐开关）", () => {
  it("无隐藏群且开关关闭时不渲染", () => {
    render(<InactiveGroupsToggle hiddenCount={0} />);
    expect(screen.queryByTestId("inactive-groups-toggle")).toBeNull();
  });

  it("有隐藏群时渲染开关，label 带计数，初始未勾选", () => {
    render(<InactiveGroupsToggle hiddenCount={2} />);
    expect(screen.getByTestId("inactive-groups-toggle").textContent).toContain("2");
    const state = (screen.getByTestId("inactive-groups-switch") as HTMLElement).dataset.state;
    expect(state).toBe("unchecked");
  });

  it("点击开关写 store 并写穿 localStorage，勾选态翻转", () => {
    render(<InactiveGroupsToggle hiddenCount={1} />);
    fireEvent.click(screen.getByTestId("inactive-groups-switch"));
    expect(useUiPrefsStore.getState().showInactiveGroups).toBe(true);
    expect(JSON.parse(localStorage.getItem("p2p-gui.ui-prefs") ?? "{}").showInactiveGroups).toBe(true);
    const state = (screen.getByTestId("inactive-groups-switch") as HTMLElement).dataset.state;
    expect(state).toBe("checked");
  });

  it("开关开启后即使无隐藏群也保持渲染（可关回）", () => {
    useUiPrefsStore.setState({ showInactiveGroups: true });
    render(<InactiveGroupsToggle hiddenCount={0} />);
    expect(screen.getByTestId("inactive-groups-toggle")).toBeTruthy();
  });
});
