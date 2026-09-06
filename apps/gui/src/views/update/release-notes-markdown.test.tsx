import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const openPageMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/ipc", () => ({
  ipc: { updateOpenReleasePage: openPageMock, updateCheck: vi.fn() },
}));

import "@/i18n";
import { ReleaseNotesMarkdown } from "./release-notes-markdown";

describe("ReleaseNotesMarkdown", () => {
  it("标题与列表按 markdown 渲染：不出现字面标记，列表项为 li", () => {
    const { container } = render(
      <ReleaseNotesMarkdown notes={"## 亮点\n- 列表项甲\n- 列表项乙"} />,
    );
    expect(
      screen.getByRole("heading", { level: 2, name: "亮点" }),
    ).toBeInTheDocument();
    const items = screen.getAllByRole("listitem");
    expect(items).toHaveLength(2);
    expect(items[0]).toHaveTextContent("列表项甲");
    expect(container.textContent).not.toContain("##");
    expect(container.textContent).not.toContain("- 列表");
  });

  it("原始 HTML 不产生对应 DOM：script/img/onerror 均退化为纯文本", () => {
    const { container } = render(
      <ReleaseNotesMarkdown
        notes={
          '<script>window.__pwned = 1</script>\n<img src=x onerror="window.__pwned = 2">'
        }
      />,
    );
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect((window as { __pwned?: number }).__pwned).toBeUndefined();
  });

  it("链接点击走 release-links 通道（update_open_release_page），不默认导航", async () => {
    openPageMock.mockReset().mockResolvedValue(undefined);
    render(
      <ReleaseNotesMarkdown
        notes="[发布页](https://example.com/release/tag/v1)"
      />,
    );
    const link = screen.getByRole("link", { name: "发布页" });
    expect(link).toHaveAttribute(
      "href",
      "https://example.com/release/tag/v1",
    );
    const event = fireEvent.click(link);
    expect(event).toBe(false);
    await waitFor(() =>
      expect(openPageMock).toHaveBeenCalledWith(
        "https://example.com/release/tag/v1",
      ),
    );
  });

  it("GFM 表格与删除线按结构化元素渲染", () => {
    const { container } = render(
      <ReleaseNotesMarkdown notes={"~~旧字段~~\n\n| 字段 | 说明 |\n| --- | --- |\n| a | b |"} />,
    );
    expect(container.querySelector("table")).toBeTruthy();
    expect(container.querySelector("del")).toBeTruthy();
  });
});
