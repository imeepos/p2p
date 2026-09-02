import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Sparkline } from "./sparkline";

const POINTS = Array.from({ length: 10 }, (_, i) => ({
  tMs: 1_000_000 + i * 5000,
  value: i,
}));

describe("Sparkline", () => {
  it("渲染折线与末点数值徽标", () => {
    render(<Sparkline points={POINTS} tone="success" label="conn" />);
    expect(screen.getByRole("img", { name: "conn" })).toBeTruthy();
    // 测试环境无 i18n 资源，t 回退为 key 原文 + 插值
    expect(screen.getByText(/dashboard.trend.now/)).toBeTruthy();
  });

  it("空序列不渲染路径也不抛错", () => {
    render(<Sparkline points={[]} tone="warning" label="empty" />);
    expect(screen.getByRole("img", { name: "empty" })).toBeTruthy();
    expect(document.querySelector("path")).toBeNull();
  });
});
