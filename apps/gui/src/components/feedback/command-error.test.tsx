import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import "@/i18n";

import { CommandErrorText } from "./command-error";

describe("CommandErrorText 内联失败原因标准呈现（C1）", () => {
  it("message 为空不渲染", () => {
    const { container } = render(<CommandErrorText message={null} />);
    expect(container.textContent).toBe("");
  });

  it("渲染前缀+原文，role=alert，复制按钮写入原文", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText } });
    render(<CommandErrorText message="boom: peer unknown" prefix="添加失败：" testId="err" />);
    const node = screen.getByTestId("err");
    expect(node.getAttribute("role")).toBe("alert");
    expect(node.textContent).toContain("添加失败");
    expect(node.textContent).toContain("boom: peer unknown");
    fireEvent.click(screen.getByRole("button", { name: /复制/ }));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith("boom: peer unknown"));
    vi.unstubAllGlobals();
  });
});
