import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";

import "@/i18n";
import { type PickerOption } from "./picker-option";
import { EntityMultiSelect } from "./entity-multi-select";

const OPTIONS: PickerOption[] = [
  { value: "p1", label: "甲", hint: "p1…tail" },
  { value: "p2", label: "乙" },
  { value: "p3", label: "丙" },
];

interface HarnessProps {
  options?: PickerOption[];
  warning?: string | null;
}

// 顶层 harness（react-hooks 编译规则：不在渲染期造组件）；已选经 DOM 断言
function MultiHarness({ options = OPTIONS, warning }: HarnessProps) {
  const [selected, setSelected] = useState<string[]>([]);
  return (
    <EntityMultiSelect
      options={options}
      selected={selected}
      onChange={setSelected}
      warning={warning}
      warningTestId="multi-warning"
      testId="multi"
    />
  );
}

describe("EntityMultiSelect 多选选择器", () => {
  it("即时搜索过滤候选列表", () => {
    render(<MultiHarness />);
    fireEvent.change(screen.getByTestId("multi-search"), { target: { value: "乙" } });
    expect(screen.getByRole("option", { name: /乙/ })).toBeTruthy();
    expect(screen.queryByRole("option", { name: /甲/ })).toBeNull();
  });

  it("已选区置顶：chip 计数与移除；重复点击同项为取消", () => {
    render(<MultiHarness />);
    fireEvent.click(screen.getByRole("option", { name: /甲/ }));
    fireEvent.click(screen.getByRole("option", { name: /丙/ }));
    const selectedArea = screen.getByTestId("multi-selected");
    expect(selectedArea.textContent).toContain("已选 2 项");
    expect(selectedArea.textContent).toContain("甲");
    expect(selectedArea.textContent).toContain("丙");
    fireEvent.click(screen.getByTestId("multi-remove-p1"));
    expect(screen.getByTestId("multi-selected").textContent).not.toContain("甲");
    fireEvent.click(screen.getByRole("option", { name: /丙/ }));
    expect(screen.queryByTestId("multi-selected")).toBeNull();
  });

  it("键盘回车切换勾选；告警文案就地呈现（现有口径由调用方传入）", () => {
    render(<MultiHarness warning="已达群成员上限 32" />);
    fireEvent.keyDown(screen.getByTestId("multi-search"), { key: "ArrowDown" });
    fireEvent.keyDown(screen.getByTestId("multi-search"), { key: "Enter" });
    expect(screen.getByTestId("multi-selected").textContent).toContain("乙");
    const warning = screen.getByTestId("multi-warning");
    expect(warning.getAttribute("role")).toBe("alert");
    expect(warning.textContent).toContain("32");
  });

  it("无匹配进入空态", () => {
    render(<MultiHarness />);
    fireEvent.change(screen.getByTestId("multi-search"), { target: { value: "无" } });
    expect(screen.getByTestId("picker-empty")).toBeTruthy();
  });
});
