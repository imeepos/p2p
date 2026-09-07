import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { PROTOCOL_DOCS } from "@/config/docs-registry";
import i18n from "@/i18n";

import { DocsView } from "./docs-view";

// R2-20 链接交互断言素材：总览篇内的真实链接（源文件自身，随文档更新不脆）
function overviewLinkHref(markdown: string, suffix: string): string {
  const line = markdown
    .split("\n")
    .find((raw) => raw.includes("](" + suffix + ")"));
  if (line === undefined) {
    throw new Error("doc fixture without link to " + suffix + ": test precondition broken");
  }
  return suffix;
}

function stubClipboard(): { writeText: ReturnType<typeof vi.fn> } {
  const writeText = vi.fn(() => Promise.resolve());
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText },
  });
  return { writeText };
}

// DOC2 验收基线：断言真实 docs/protocol 文件内容渲染进 DOM（禁占位文本）。
// 断言素材全部从 ?raw 真实文档动态提取——文档更新测试不脆，且天然排除
// 假 fixture；标题/正文行均来自仓库单源文件本身。

function firstH1(markdown: string): string {
  const heading = markdown.split("\n").find((line) => line.startsWith("# "));
  if (heading === undefined) {
    throw new Error("doc fixture without H1: test precondition broken");
  }
  return heading.replace(/^#\s+/, "").trim();
}

// 取一段真实正文：跳过标题/表格/引用/列表/链接/行内格式行（这些元素的
// 渲染文本与源行不一致），其余行应逐字出现在渲染文本里（软换行合并
// 不影响行内连续性）。找不到即 throw，测试前置破坏显式暴露不静默。
function realBodyLine(markdown: string): string {
  const line = markdown
    .split("\n")
    .map((raw) => raw.trim())
    .find(
      (raw) =>
        raw.length >= 8 &&
        !["#", "|", ">", "-", "*", "["].some((mark) => raw.startsWith(mark)) &&
        !["`", "*", "].(", "<"].some((mark) => raw.includes(mark)),
    );
  if (line === undefined) {
    throw new Error("doc fixture without prose line: test precondition broken");
  }
  return line;
}

afterEach(() => cleanup());

describe("/docs 协议文档页（DOC2）", () => {
  it("默认渲染总览篇真实内容进 DOM（H1 与正文行）", () => {
    const { container } = render(<DocsView />);
    const overview = PROTOCOL_DOCS[0];
    expect(
      screen.getByRole("heading", { level: 1, name: firstH1(overview.markdown) }),
    ).toBeInTheDocument();
    expect(container.textContent).toContain(realBodyLine(overview.markdown));
  });

  it("目录五篇逐一可切换，切换后渲染对应真实文档", () => {
    expect(PROTOCOL_DOCS).toHaveLength(5);
    const { container } = render(<DocsView />);
    for (const doc of PROTOCOL_DOCS) {
      fireEvent.click(screen.getByRole("button", { name: doc.title }));
      expect(
        screen.getByRole("heading", { level: 1, name: firstH1(doc.markdown) }),
      ).toBeInTheDocument();
      expect(container.textContent).toContain(realBodyLine(doc.markdown));
    }
  });

  it("页面 chrome 经 i18n 渲染（页标题/目录标签）", () => {
    render(<DocsView />);
    expect(
      screen.getByRole("heading", { level: 1, name: i18n.t("docs.title") }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("navigation", { name: i18n.t("docs.toc") }),
    ).toBeInTheDocument();
  });

  it("R2-20 跨文链接点击切换到目标篇（人话标题，应用内跳转）", () => {
    const overview = PROTOCOL_DOCS[0];
    const target = PROTOCOL_DOCS.find((d) => d.id === "wire-format")!;
    overviewLinkHref(overview.markdown, "wire-format.md");
    const { container } = render(<DocsView />);
    const link = screen.getByRole("link", { name: target.title });
    expect(link.getAttribute("title")).toBe("wire-format.md");
    fireEvent.click(link);
    expect(
      screen.getByRole("heading", { level: 1, name: firstH1(target.markdown) }),
    ).toBeInTheDocument();
    expect(container.textContent).toContain(realBodyLine(target.markdown));
    // 目录 aria-current 随跨文跳转同步到目标篇
    expect(
      screen.getByRole("button", { name: target.title }).getAttribute("aria-current"),
    ).toBe("page");
  });

  it("R2-20 不可达仓库路径链接：渲染短名+title 保留原路径，点击复制路径给反馈", async () => {
    const overview = PROTOCOL_DOCS[0];
    const href = overviewLinkHref(overview.markdown, "../design/wire-protocol.md");
    const { writeText } = stubClipboard();
    render(<DocsView />);
    const link = screen.getByRole("link", { name: "wire-protocol" });
    expect(link.getAttribute("title")).toBe(href);
    expect(link.getAttribute("href")).toBe(href);
    expect(link.textContent).not.toBe(href);
    fireEvent.click(link);
    await vi.waitFor(() => expect(writeText).toHaveBeenCalledWith(href));
  });

  it("R2-21 长文滚动出返回顶部按钮，点击回到顶部即隐", () => {
    const overview = PROTOCOL_DOCS[0];
    render(<DocsView />);
    expect(screen.queryByTestId("docs-back-top")).toBeNull();
    const section = screen.getByRole("region", { name: overview.title });
    Object.defineProperty(section, "scrollTop", { value: 800, writable: true, configurable: true });
    fireEvent.scroll(section);
    const button = screen.getByTestId("docs-back-top");
    expect(button.getAttribute("aria-label")).toBe(i18n.t("docs.backToTop"));
    fireEvent.click(button);
    expect(section.scrollTop).toBe(0);
    expect(screen.queryByTestId("docs-back-top")).toBeNull();
  });
});
