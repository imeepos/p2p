import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import "@/i18n";

import { AddAddressDialog } from "./add-address-dialog";

function openDialog(existing: string[] = []) {
  render(
    <AddAddressDialog
      existing={existing}
      saving={false}
      onAdd={vi.fn(async () => true)}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "添加地址" }));
}

// F13：引导地址弹窗输入必须配可见标签，占位符只放示例；
// 校验失败就地 role=alert（口径同 chat-friend-add-dialog）。
describe("AddAddressDialog（F13）", () => {
  it("地址输入带可见标签并以 htmlFor 关联，占位符只放示例", () => {
    openDialog();
    const input = screen.getByLabelText("引导地址");
    expect(input.tagName).toBe("INPUT");
    expect(input).toHaveAttribute("placeholder", "192.168.1.10/u3400");
  });

  it("非法输入 aria-invalid 且 role=alert 就地提示，修正后提示消失", () => {
    openDialog(["192.168.1.10/u3400"]);
    const input = screen.getByLabelText("引导地址");
    fireEvent.change(input, { target: { value: "not-an-addr" } });
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(input.getAttribute("aria-describedby")).toBe(
      "discovery-add-addr-error",
    );
    expect(screen.getByRole("alert").textContent).toContain("地址格式");
    fireEvent.change(input, { target: { value: "192.168.1.11/u3400" } });
    expect(screen.queryByRole("alert")).toBeNull();
    expect(input.getAttribute("aria-invalid")).toBeNull();
  });
});
