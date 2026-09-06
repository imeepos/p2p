import type { ComponentPropsWithoutRef } from "react";

import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";

import { openReleasePage } from "./release-links";

// 标题保留语义层级（读屏按标题导航），字号统一压到卡片正文一档，只以
// 字重分层；首元素去顶距贴合容器内边距。
function markdownHeading(Tag: "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
  return function MarkdownHeading({
    children,
  }: ComponentPropsWithoutRef<"h2">) {
    return (
      <Tag className="text-sm font-semibold mt-2 mb-1 first:mt-0">
        {children}
      </Tag>
    );
  };
}

// 链接沿用发布链接唯一通道（update_open_release_page 白名单），拦截默认
// 导航避免 webview 内跳转；href 保留给语义与无障碍。
function MarkdownLink({ children, href }: ComponentPropsWithoutRef<"a">) {
  return (
    <a
      href={href}
      className="underline underline-offset-2 break-words"
      onClick={(event) => {
        event.preventDefault();
        void openReleasePage(href ?? null);
      }}
    >
      {children}
    </a>
  );
}

function MarkdownP(props: ComponentPropsWithoutRef<"p">) {
  return (
    <p
      {...props}
      className="text-sm leading-relaxed break-words my-1 first:mt-0 last:mb-0"
    />
  );
}

function MarkdownUl(props: ComponentPropsWithoutRef<"ul">) {
  return <ul {...props} className="list-disc pl-5 my-1 space-y-0.5" />;
}

function MarkdownOl(props: ComponentPropsWithoutRef<"ol">) {
  return <ol {...props} className="list-decimal pl-5 my-1 space-y-0.5" />;
}

function MarkdownLi(props: ComponentPropsWithoutRef<"li">) {
  return <li {...props} className="text-sm leading-relaxed break-words" />;
}

function MarkdownPre(props: ComponentPropsWithoutRef<"pre">) {
  return (
    <pre
      {...props}
      className="bg-muted rounded-md p-2 my-1 overflow-x-auto font-mono text-xs"
    />
  );
}

function MarkdownCode(props: ComponentPropsWithoutRef<"code">) {
  return <code {...props} className="font-mono text-xs break-words" />;
}

function MarkdownBlockquote(props: ComponentPropsWithoutRef<"blockquote">) {
  return (
    <blockquote
      {...props}
      className="border-l-2 border-border pl-3 my-1 text-muted-foreground"
    />
  );
}

// 发布说明最小排版映射：标题/列表/段落/代码/引用/链接。react-markdown
// 默认不渲染原始 HTML（无 rehype-raw），恶意片段退化为纯文本。
const COMPONENTS: Components = {
  h1: markdownHeading("h1"),
  h2: markdownHeading("h2"),
  h3: markdownHeading("h3"),
  h4: markdownHeading("h4"),
  h5: markdownHeading("h5"),
  h6: markdownHeading("h6"),
  p: MarkdownP,
  ul: MarkdownUl,
  ol: MarkdownOl,
  li: MarkdownLi,
  pre: MarkdownPre,
  code: MarkdownCode,
  blockquote: MarkdownBlockquote,
  a: MarkdownLink,
};

// 发布说明 markdown 渲染块：表格/删除线/任务列表等 GFM 语法经 remark-gfm。
export function ReleaseNotesMarkdown({ notes }: { notes: string }) {
  return (
    <div className="text-sm">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={COMPONENTS}>
        {notes}
      </ReactMarkdown>
    </div>
  );
}
