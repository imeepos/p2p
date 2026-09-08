import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { SegmentedControl } from "./segmented-control";

// 分段控件原语：aria-pressed 单选语义、计数渲染、点击回调。

describe("SegmentedControl", () => {
  it("选中项 aria-pressed=true，其余 false", () => {
    render(
      <SegmentedControl
        value="pending"
        onChange={vi.fn()}
        ariaLabel="视图切换"
        options={[
          { value: "pending", label: "待处理", count: 3 },
          { value: "history", label: "历史消息" },
        ]}
      />,
    );
    expect(
      screen.getByTestId("segmented-pending").getAttribute("aria-pressed"),
    ).toBe("true");
    expect(
      screen.getByTestId("segmented-history").getAttribute("aria-pressed"),
    ).toBe("false");
    expect(screen.getByRole("group").getAttribute("aria-label")).toBe("视图切换");
  });

  it("计数渲染为等宽数字，无计数不渲染", () => {
    render(
      <SegmentedControl
        value="pending"
        onChange={vi.fn()}
        ariaLabel="视图切换"
        options={[
          { value: "pending", label: "待处理", count: 12 },
          { value: "history", label: "历史消息" },
        ]}
      />,
    );
    const pending = screen.getByTestId("segmented-pending");
    expect(pending.textContent).toContain("12");
    expect(screen.getByTestId("segmented-history").textContent).toBe("历史消息");
  });

  it("点击未选中项触发 onChange 且只触发一次", () => {
    const onChange = vi.fn();
    render(
      <SegmentedControl
        value="pending"
        onChange={onChange}
        ariaLabel="视图切换"
        options={[
          { value: "pending", label: "待处理" },
          { value: "history", label: "历史消息" },
        ]}
      />,
    );
    fireEvent.click(screen.getByTestId("segmented-history"));
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith("history");
  });
});
