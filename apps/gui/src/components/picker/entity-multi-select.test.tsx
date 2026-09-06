import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import "@/i18n";
import { type PickerOption } from "./picker-option";
import { EntityMultiSelect } from "./entity-multi-select";

const OPTIONS: PickerOption[] = [
  { value: "p1", label: "甲", hint: "p1…tail" },
  { value: "p2", label: "乙" },
  { value: "p3", label: "丙" },
];

function Harness(props: { warning?: string | null }): {
  current: string[];
} {
  const box: { current: string[] } = { current: [] };
  function Inner() {
    const [selected, setSelected] = useState<string[]>([]);
    box.current = selected;
    return (
      <EntityMultiSelect
        options={OPTIONS}
        selected={selected}
        onChange={setSelected}
        warning={props.warning}
        warningTestId="multi-warning"
        testId="multi"
      />
    );
  }
  render(<Inner />);
  return box;
}

describe("EntityMultiSelect 多选选择器", () => {
  it("即时搜索过滤候选列表", () => {
    Harness({});
    fireEvent.change(screen.getByTestId("multi-search"), { target: { value: "乙" } });
    expect(screen.getByRole("option", { name: /乙/ })).toBeTruthy();
    expect(screen.queryByRole("option", { name: /甲/ })).toBeNull();
  });

  it("已选区置顶：chip 计数与移除；列表勾选态同步", () => {
    const box = Harness({});
    fireEvent.click(screen.getByRole("option", { name: /甲/ }));
    fireEvent.click(screen.getByRole("option", { name: /丙/ }));
    expect(box.current).toEqual(["p1", "p3"]);
    const selectedArea = screen.getByTestId("multi-selected");
    expect(selectedArea.textContent).toContain("已选 2 项");
    expect(selectedArea.textContent).toContain("甲");
    fireEvent.click(screen.getByTestId("multi-remove-p1"));
    expect(box.current).toEqual(["p3"]);
  });

  it("键盘回车切换勾选；告警文案就地呈现（现有口径由调用方传入）", () => {
    const box = Harness({ warning: "已达群成员上限 32" });
    fireEvent.keyDown(screen.getByTestId("multi-search"), { key: "ArrowDown" });
    fireEvent.keyDown(screen.getByTestId("multi-search"), { key: "Enter" });
    expect(box.current).toEqual(["p2"]);
    const warning = screen.getByTestId("multi-warning");
    expect(warning.getAttribute("role")).toBe("alert");
    expect(warning.textContent).toContain("32");
  });

  it("无匹配进入空态；重复选择同一项为取消", () => {
    const box = Harness({});
    fireEvent.change(screen.getByTestId("multi-search"), { target: { value: "无" } });
    expect(screen.getByTestId("picker-empty")).toBeTruthy();
    fireEvent.change(screen.getByTestId("multi-search"), { target: { value: "" } });
    fireEvent.click(screen.getByRole("option", { name: /乙/ }));
    fireEvent.click(screen.getByRole("option", { name: /乙/ }));
    expect(box.current).toEqual([]);
  });
});
