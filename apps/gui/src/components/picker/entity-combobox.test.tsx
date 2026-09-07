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

interface HarnessProps {
  options?: PickerOption[];
  loading?: boolean;
  error?: string | null;
  onRetry?: () => void;
  testId?: string;
  clearable?: boolean;
  placeholder?: string;
}

// 顶层 harness（react-hooks 编译规则：不在渲染期造组件）；选中态经 DOM 断言
function ComboHarness({
  options = OPTIONS,
  loading,
  error,
  onRetry,
  testId = "picker",
  clearable,
  placeholder,
}: HarnessProps) {
  const [value, setValue] = useState<string | null>(null);
  return (
    <EntityCombobox
      options={options}
      value={value}
      onChange={setValue}
      loading={loading}
      error={error}
      onRetry={onRetry}
      testId={testId}
      clearable={clearable}
      placeholder={placeholder}
    />
  );
}

function openPanel(testId = "picker"): void {
  fireEvent.click(screen.getByTestId(testId));
}

describe("EntityCombobox 单选选择器", () => {
  it("展开后即时搜索过滤选项：命中保留，未命中隐藏", () => {
    render(<ComboHarness />);
    openPanel();
    expect(screen.getByRole("option", { name: /小圆/ })).toBeTruthy();
    fireEvent.change(screen.getByTestId("picker-search"), { target: { value: "gamma" } });
    expect(screen.getByRole("option", { name: /远端 gamma/ })).toBeTruthy();
    expect(screen.queryByRole("option", { name: /小圆/ })).toBeNull();
  });

  it("键盘上下移动高亮、回车选中并收起面板", () => {
    render(<ComboHarness />);
    openPanel();
    fireEvent.keyDown(screen.getByTestId("picker-search"), { key: "ArrowDown" });
    fireEvent.keyDown(screen.getByTestId("picker-search"), { key: "Enter" });
    expect(screen.queryByTestId("picker-panel")).toBeNull();
    expect(screen.getByTestId("picker").textContent).toContain("节点 beta");
  });

  it("加载态显示加载提示；错误态显示失败与重试入口并可触发", () => {
    const onRetry = vi.fn();
    const { unmount } = render(<ComboHarness options={[]} loading testId="picker-l" />);
    openPanel("picker-l");
    expect(screen.getByTestId("picker-loading")).toBeTruthy();
    unmount();
    render(<ComboHarness options={[]} error="boom" onRetry={onRetry} testId="picker-e" />);
    fireEvent.click(screen.getByTestId("picker-e"));
    expect(screen.getByTestId("picker-error")).toBeTruthy();
    fireEvent.click(screen.getByTestId("picker-retry"));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it("无匹配进入空态；选中后可清空（触发器回到占位）", () => {
    render(<ComboHarness />);
    openPanel();
    fireEvent.change(screen.getByTestId("picker-search"), { target: { value: "不存在" } });
    expect(screen.getByTestId("picker-empty")).toBeTruthy();
    fireEvent.change(screen.getByTestId("picker-search"), { target: { value: "" } });
    fireEvent.click(screen.getByRole("option", { name: /小圆/ }));
    expect(screen.getByTestId("picker").textContent).toContain("小圆");
    fireEvent.click(screen.getByTestId("picker-clear"));
    expect(screen.getByTestId("picker").textContent).not.toContain("小圆");
  });

  it("选项副行展示缩略标识", () => {
    render(<ComboHarness />);
    openPanel();
    expect(
      screen.getByRole("option", { name: /小圆/ }).textContent,
    ).toContain(shortPeerId("p-alpha-full"));
  });

  it("触发器挂 id：Label htmlFor 点击命中触发器并展开面板", () => {
    render(
      <>
        {/* biome-ignore lint/a11y: 断言目标就是 label 关联行为 */}
        <label htmlFor="entity-combobox">从列表选择</label>
        <ComboHarness />
      </>,
    );
    fireEvent.click(screen.getByText("从列表选择"));
    expect(screen.getByTestId("picker-panel")).toBeTruthy();
  });

  it("选中后焦点归还触发器；Esc 收起面板同样归还且不冒泡出容器", () => {
    const onWrapperKeyDown = vi.fn();
    const onDocumentKeyDown = vi.fn();
    document.addEventListener("keydown", onDocumentKeyDown);
    try {
      render(
        <div onKeyDown={onWrapperKeyDown}>
          <ComboHarness />
        </div>,
      );
      // Esc 收面板：焦点回触发器，且对 document 级监听（Dialog 关闭路径）不穿透
      openPanel();
      fireEvent.keyDown(screen.getByTestId("picker-search"), { key: "Escape" });
      expect(screen.queryByTestId("picker-panel")).toBeNull();
      expect(screen.getByTestId("picker")).toEqual(document.activeElement);
      expect(onWrapperKeyDown).not.toHaveBeenCalled();
      expect(onDocumentKeyDown).not.toHaveBeenCalled();

      // 回车选中：焦点同样归还；事件本身正常冒泡（不额外吞键）
      openPanel();
      fireEvent.keyDown(screen.getByTestId("picker-search"), { key: "Enter" });
      expect(screen.queryByTestId("picker-panel")).toBeNull();
      expect(screen.getByTestId("picker")).toEqual(document.activeElement);
      expect(onWrapperKeyDown).toHaveBeenCalledTimes(1);
    } finally {
      document.removeEventListener("keydown", onDocumentKeyDown);
    }
  });

  it("clearable=false 时选中后不出清空叉；placeholder 覆盖节点语境默认文案", () => {
    render(<ComboHarness clearable={false} placeholder="选择模型" />);
    expect(screen.getByTestId("picker").textContent).toContain("选择模型");
    openPanel();
    fireEvent.click(screen.getByRole("option", { name: /小圆/ }));
    expect(screen.getByTestId("picker").textContent).toContain("小圆");
    expect(screen.queryByTestId("picker-clear")).toBeNull();
  });
});
