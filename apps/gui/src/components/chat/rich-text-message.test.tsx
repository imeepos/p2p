import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { RichTextMessage } from "./rich-text-message";

function renderText(text: string) {
  return render(<RichTextMessage text={text} />);
}

const xssWindow = () => window as unknown as Record<string, unknown>;

describe("RichTextMessage·GFM", () => {
  it("表格渲染表头与单元格", () => {
    const { container } = renderText(["| a | b |", "| - | - |", "| 1 | 2 |"].join("\n"));
    expect(container.querySelector("thead th")?.textContent).toBe("a");
    expect(container.querySelectorAll("tbody td")).toHaveLength(2);
  });

  it("任务列表渲染勾选态", () => {
    const { container } = renderText("- [x] 已完成\n- [ ] 待办");
    const boxes = container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]');
    expect(boxes).toHaveLength(2);
    expect(boxes[0].checked).toBe(true);
    expect(boxes[1].checked).toBe(false);
  });

  it("删除线渲染 del 元素", () => {
    const { container } = renderText("~~作废~~");
    expect(container.querySelector("del")?.textContent).toBe("作废");
  });

  it("独立 URL 自动成链", () => {
    const { container } = renderText("看 https://example.com/doc 文档");
    const anchor = container.querySelector("a");
    expect(anchor?.getAttribute("href")).toBe("https://example.com/doc");
  });

  it("单独换行按原文保留（IM 换行语义，段落 pre-wrap 视觉换行）", () => {
    const { container } = renderText("第一行\n第二行");
    expect(container.querySelector("p")?.textContent).toBe("第一行\n第二行");
  });
});

describe("RichTextMessage·数学公式", () => {
  it("行内公式渲染 KaTeX", () => {
    const { container } = renderText("质能方程 $E=mc^2$ 著名");
    expect(container.querySelector(".katex")).toBeTruthy();
  });

  it("块级公式渲染 katex-display", () => {
    const { container } = renderText("$$\n\\int_0^1 x^2\\,dx = \\frac{1}{3}\n$$");
    expect(container.querySelector(".katex-display")).toBeTruthy();
  });
});

describe("RichTextMessage·代码高亮", () => {
  it("已知语言生成 hljs token", () => {
    const code = ["```js", "const a = 1;", "```"].join("\n");
    const { container } = renderText(code);
    expect(container.querySelector("pre code .hljs-keyword")).toBeTruthy();
  });

  it("未知语言不报错且代码完整可读", () => {
    const code = ["```nosuchlang", "plain content here", "```"].join("\n");
    const { container } = renderText(code);
    expect(container.querySelector("pre")?.textContent).toContain("plain content here");
  });

  it("无语言围栏代码原样展示", () => {
    const code = ["```", "const a = 1;", "```"].join("\n");
    const { container } = renderText(code);
    expect(container.querySelector("pre code")?.textContent).toContain("const a = 1;");
  });
});

describe("RichTextMessage·XSS 消毒", () => {
  it("裸 script 标签文本化，不进 DOM 不执行", () => {
    const { container } = renderText("<script>window.__p2p_xss = 1;</script>");
    expect(container.querySelector("script")).toBeNull();
    expect(xssWindow().__p2p_xss).toBeUndefined();
  });

  it("img onerror 事件属性不产生元素", () => {
    const { container } = renderText('<img src=x onerror="window.__p2p_xss = 1">');
    expect(container.querySelector("img")).toBeNull();
    expect(xssWindow().__p2p_xss).toBeUndefined();
  });

  it("javascript: 链接退化为纯文本，无可点锚点", () => {
    const { container } = renderText("[点我](javascript:alert(1))");
    expect(container.querySelector("a")).toBeNull();
    expect(container.textContent).toContain("点我");
  });

  it("data: 协议链接默认拒绝", () => {
    const { container } = renderText("[点我](data:text/html,<b>hi</b>)");
    expect(container.querySelector("a")).toBeNull();
    expect(container.textContent).toContain("点我");
  });

  it("vbscript: 协议链接默认拒绝", () => {
    const { container } = renderText("[点我](vbscript:msgbox(1))");
    expect(container.querySelector("a")).toBeNull();
    expect(container.textContent).toContain("点我");
  });

  it("https 链接不受消毒影响", () => {
    const { container } = renderText("[正常](https://example.com/a?x=1)");
    expect(container.querySelector("a")?.getAttribute("href")).toBe("https://example.com/a?x=1");
  });
});
