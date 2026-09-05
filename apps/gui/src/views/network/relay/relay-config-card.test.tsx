import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import "@/i18n";

import { RelayConfigCard } from "./relay-config-card";

const noopSave = () => vi.fn().mockResolvedValue(undefined)();

// 回归：FactoryDefaultsNotice 曾挂在 FormProvider 之外，
// useFormContext() 返回 null，解构 control 时整页抛
// "Cannot destructure property 'control'" 并落入 ErrorBoundary。
// 本文件按 relay 页真实组合挂载（不带任何外部 provider），
// 修复前必红；防再犯：该组件换页面复用时必须仍在表单上下文内。
describe("RelayConfigCard", () => {
  it("空地址列表挂载不崩溃，显示出厂默认提示", () => {
    const { container } = render(
      <RelayConfigCard relayAddrs={[]} onSave={noopSave} />,
    );
    expect(screen.getByText("中继地址配置")).toBeInTheDocument();
    expect(container.textContent).not.toContain("界面出错了");
    expect(
      screen.getByText(/列表为空时使用出厂默认中继端点/),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "恢复出厂默认" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  });

  it("恢复出厂默认写入两行地址、置脏并隐藏提示", async () => {
    render(<RelayConfigCard relayAddrs={[]} onSave={noopSave} />);
    fireEvent.click(screen.getByRole("button", { name: "恢复出厂默认" }));
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "保存" })).toBeEnabled();
    });
    const rows = screen.getAllByPlaceholderText("192.168.1.10/u3403");
    expect(rows).toHaveLength(2);
    expect(
      screen.queryByRole("button", { name: "恢复出厂默认" }),
    ).not.toBeInTheDocument();
  });

  it("已有地址时不显示出厂默认提示", () => {
    render(
      <RelayConfigCard
        relayAddrs={["43.240.223.138/u3403"]}
        onSave={noopSave}
      />,
    );
    expect(
      screen.queryByRole("button", { name: "恢复出厂默认" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  });
});
