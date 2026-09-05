// 配置面板可访问性测试（P2 页面打磨）：下拉触发器经 aria-labelledby
// 关联类别标签，读屏可念出「模型」而非裸值。
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

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

  it("非 select 或空目录的选项不渲染行", () => {
    seed([{ id: "toggle", name: "T", type: "boolean", currentValue: true }]);
    const { container } = render(<ConfigPanel />);
    expect(container.querySelector("[data-testid^='acp-config-option']")).toBeNull();
  });
});
