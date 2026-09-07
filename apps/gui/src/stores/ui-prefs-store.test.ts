import { beforeEach, describe, expect, it, vi } from "vitest";

// 每用例重载模块：验证 localStorage 初值读路径与损坏告警回退（不静默）。
async function freshStore() {
  return await import("@/stores/ui-prefs-store");
}

beforeEach(() => {
  vi.resetModules();
  localStorage.clear();
});

describe("ui-prefs：showInactiveGroups 存档", () => {
  it("空存档默认 false（隐藏已退群）", async () => {
    const { useUiPrefsStore } = await freshStore();
    expect(useUiPrefsStore.getState().showInactiveGroups).toBe(false);
  });

  it("set 写穿 localStorage 并入 state；重载读到 true", async () => {
    const { useUiPrefsStore } = await freshStore();
    useUiPrefsStore.getState().setShowInactiveGroups(true);
    expect(useUiPrefsStore.getState().showInactiveGroups).toBe(true);
    expect(JSON.parse(localStorage.getItem("p2p-gui.ui-prefs") ?? "{}")).toEqual({
      showInactiveGroups: true,
    });
    const reloaded = await freshStore();
    expect(reloaded.useUiPrefsStore.getState().showInactiveGroups).toBe(true);
  });

  it("损坏 JSON 显式告警并回 false", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    localStorage.setItem("p2p-gui.ui-prefs", "{not-json");
    const { useUiPrefsStore } = await freshStore();
    expect(useUiPrefsStore.getState().showInactiveGroups).toBe(false);
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });

  it("非布尔值视为 false", async () => {
    localStorage.setItem(
      "p2p-gui.ui-prefs",
      JSON.stringify({ showInactiveGroups: "yes" }),
    );
    const { useUiPrefsStore } = await freshStore();
    expect(useUiPrefsStore.getState().showInactiveGroups).toBe(false);
  });
});
