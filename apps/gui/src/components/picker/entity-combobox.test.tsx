import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import "@/i18n";
import { shortPeerId, type PickerOption } from "./picker-option";
import { EntityCombobox } from "./entity-combobox";

const OPTIONS: PickerOption[] = [
  { value: "p-alpha", label: "小圆", hint: shortPeerId("p-alpha-full") },
  { value: "p-beta", label: "节点 beta", hint: shortPeerId("p-beta-full") },
  { value: "p-gamma", label: "远端 gamma" },
];

function Harness(props: {
  loading?: boolean;
  error?: string | null;
  onRetry?: () => void;
}): { current: string | null } {
  const box: { current: string | null } = { current: null };
  function Inner() {
    const [value, setValue] = useState<string | null>(null);
    box.current = value;
    return (
      <EntityCombobox
        options={OPTIONS}
        value={value}
        onChange={setValue}
        loading={props.loading}
        error={props.error}
        onRetry={props.onRetry}
        testId="picker"
      />
    );
  }
  render(<Inner />);
  return box;
}

function openPanel(): void {
  fireEvent.click(screen.getByTestId("picker"));
}

describe("EntityCombobox 单选选择器", () => {
  it("展开后即时搜索过滤选项：命中保留，未命中隐藏", () => {
    Harness({});
    openPanel();
    expect(screen.getByRole("option", { name: /小圆/ })).toBeTruthy();
    fireEvent.change(screen.getByTestId("picker-search"), { target: { value: "gamma" } });
    expect(screen.getByRole("option", { name: /远端 gamma/ })).toBeTruthy();
    expect(screen.queryByRole("option", { name: /小圆/ })).toBeNull();
  });

  it("键盘上下移动高亮、回车选中并收起面板", () => {
    const box = Harness({});
    openPanel();
    fireEvent.keyDown(screen.getByTestId("picker-search"), { key: "ArrowDown" });
    fireEvent.keyDown(screen.getByTestId("picker-search"), { key: "Enter" });
    expect(box.current).toBe("p-beta");
    expect(screen.queryByTestId("picker-panel")).toBeNull();
  });

  it("加载态显示加载提示；错误态显示失败与重试入口并可触发", () => {
    const onRetry = vi.fn();
    const { unmount } = render(
      <EntityCombobox options={[]} value={null} onChange={vi.fn()} loading testId="picker-l" />,
    );
    fireEvent.click(screen.getByTestId("picker-l"));
    expect(screen.getByTestId("picker-loading")).toBeTruthy();
    unmount();
    render(
      <EntityCombobox
        options={[]}
        value={null}
        onChange={vi.fn()}
        error="boom"
        onRetry={onRetry}
        testId="picker-e"
      />,
    );
    fireEvent.click(screen.getByTestId("picker-e"));
    expect(screen.getByTestId("picker-error")).toBeTruthy();
    fireEvent.click(screen.getByTestId("picker-retry"));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it("无匹配进入空态；选中后可清空（onChange(null)）", () => {
    const box = Harness({});
    openPanel();
    fireEvent.change(screen.getByTestId("picker-search"), { target: { value: "不存在" } });
    expect(screen.getByTestId("picker-empty")).toBeTruthy();
    fireEvent.change(screen.getByTestId("picker-search"), { target: { value: "" } });
    fireEvent.click(screen.getByRole("option", { name: /小圆/ }));
    fireEvent.click(screen.getByTestId("picker-clear"));
    expect(box.current).toBeNull();
  });

  it("选项副行展示缩略标识；触发器回显人可读名", () => {
    const box = Harness({});
    openPanel();
    const row = screen.getByRole("option", { name: /小圆/ });
    expect(row.textContent).toContain(shortPeerId("p-alpha-full"));
    fireEvent.click(row);
    expect(screen.getByTestId("picker").textContent).toContain("小圆");
    expect(box.current).toBe("p-alpha");
  });
});
