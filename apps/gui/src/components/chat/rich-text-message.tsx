import { Component, memo, type ComponentProps, type ReactNode } from "react";
import DOMPurify from "dompurify";
import ReactMarkdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import rehypeKatex from "rehype-katex";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";

import "highlight.js/styles/github.css";
import "katex/dist/katex.min.css";
import "./rich-text.css";

// 协议白名单：未知协议一律拒绝（fallback 到纯文本展示，见 sanitizeUrl）。
const SAFE_URL_PROTOCOLS = /^(?:https?:|mailto:|asset:)/i;

// 链接消毒两层：DOMPurify 属性级消毒剔除危险 scheme（javascript:/data:/vbscript: 等，
// 被剥除时 href 属性整体消失），再过显式协议白名单兜底未知协议。返回空串时
// react-markdown 不输出 href，锚点退化为纯文本。
function sanitizeUrl(url: string): string {
  const clean = DOMPurify.sanitize(`<a href="${url}"></a>`, {
    ALLOWED_TAGS: ["a"],
    ALLOWED_ATTR: ["href"],
    KEEP_CONTENT: false,
  });
  const doc = new DOMParser().parseFromString(clean, "text/html");
  const href = doc.querySelector("a")?.getAttribute("href") ?? "";
  return SAFE_URL_PROTOCOLS.test(href) ? href : "";
}

// 插件数组模块级固定：引用稳定，避免每次渲染重建导致 remark 全量重解析。
const REMARK_PLUGINS = [remarkGfm, remarkMath];
// IM 换行语义：不做 remark 级换行转换（remark-breaks 不在固定技术栈内），
// markdown 软换行默认以 \n 文本保留进 DOM，配合 rich-text.css 段落
// white-space: pre-wrap 实现视觉换行，与旧纯文本行为对齐。
// rehype-highlight 默认不探测语言（detect=false）：无法识别的 language-* 静默跳过，
// 代码原样可读；rehype-katex 解析失败默认渲染错误标记 span，不抛异常。
const REHYPE_PLUGINS = [rehypeKatex, rehypeHighlight];

// 消毒后无有效 href 的链接（协议被拒）：退化为纯文本 span，不输出可点锚点。
// node 是 react-markdown 额外透传的 hast 节点，显式接住避免漏进 DOM 属性。
function SafeAnchor({ href, children, node: _hastNode, ...rest }: ComponentProps<"a"> & { node?: unknown }) {
  if (!href) return <span>{children}</span>;
  return (
    <a href={href} {...rest}>
      {children}
    </a>
  );
}

const COMPONENTS = { a: SafeAnchor };

function RichTextMessageInner({ text }: { text: string }) {
  return (
    <div className="chat-rich-text break-words" data-testid="chat-rich-text">
      <ReactMarkdown
        components={COMPONENTS}
        remarkPlugins={REMARK_PLUGINS}
        rehypePlugins={REHYPE_PLUGINS}
        urlTransform={sanitizeUrl}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}

// 渲染失败回退纯文本（渲染管线异常的兜底路径），console.error 留可观测信号。
class RichTextBoundary extends Component<
  { text: string; children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch(error: unknown) {
    console.error("[chat] 富文本渲染失败，回退纯文本", error);
  }

  render() {
    if (this.state.failed) {
      return <p className="whitespace-pre-wrap break-words">{this.props.text}</p>;
    }
    return this.props.children;
  }
}

// memo：消息列表滚动/相邻消息更新时，text 未变的气泡跳过 markdown 重解析。
export const RichTextMessage = memo(function RichTextMessage({ text }: { text: string }) {
  return (
    <RichTextBoundary text={text}>
      <RichTextMessageInner text={text} />
    </RichTextBoundary>
  );
});
