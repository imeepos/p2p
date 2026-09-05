// 配置面板可访问性测试（P2 页面打磨）：下拉触发器经 aria-labelledby
// 关联类别标签，读屏可念出「模型」而非裸值。
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useAcpStore } from "@/acp/acp-store";
import type { ConfigOption } from "@/acp/protocol";
import { ConfigPanel } from "./config-panel";

await import("@/i18n");

const SID = "s-cfg";

const MODEL_OPTION: ConfigOption = {
  id: "model",
  name: "Model",
  category: "model",
  type: "select",
  currentValue: "mock-model-a",
  options: [
    { value: "mock-model-a", name: "Mock Model A" },
    { value: "mock-model-b", name: "Mock Model B" },
  ],
};

function seed(options: ConfigOption[]) {
  useAcpStore.setState((s) => ({
    activeSessionId: SID,
    interactions: {
      ...s.interactions,
      [SID]: { ...(s.interactions[SID] ?? { permissions: [], usage: null }), configOptions: options },
    },
  }));
}

beforeEach(() => {
  useAcpStore.getState().resetConsoleState();
});

describe("ConfigPanel 可访问性", () => {
  it("下拉触发器 aria-labelledby 指向类别标签（模型）", () => {
    seed([MODEL_OPTION]);
    render(<ConfigPanel />);
    const trigger = screen.getByTestId("acp-config-option-model");
    const labelId = trigger.getAttribute("aria-labelledby");
    expect(labelId).toBe("acp-config-label-model");
    expect(document.getElementById(labelId ?? "")?.textContent).toBe("模型");
  });

  it("未识别 type 的选项不渲染行", () => {
    seed([{ id: "future", name: "F", type: "hologram", currentValue: "x" }]);
    const { container } = render(<ConfigPanel />);
    expect(container.querySelector("[data-testid^='acp-config-option']")).toBeNull();
  });

  it("boolean 配置渲染开关并纳入下发链路（setConfigOption 收布尔）", async () => {
    const spy = vi.spyOn(useAcpStore.getState(), "setConfigOption").mockResolvedValue();
    seed([{ id: "verbose", name: "Verbose", type: "boolean", currentValue: true }]);
    render(<ConfigPanel />);
    const sw = screen.getByTestId("acp-config-option-verbose");
    expect(sw.getAttribute("aria-checked")).toBe("true");
    expect(sw.getAttribute("aria-labelledby")).toBe("acp-config-label-verbose");
    fireEvent.click(sw);
    expect(spy).toHaveBeenCalledWith("verbose", false);
    spy.mockRestore();
  });
});
