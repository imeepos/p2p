import { createContext, useContext, type ReactNode } from "react";
import type { ComponentPropsWithoutRef } from "react";

import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";

import { PROTOCOL_DOCS } from "@/config/docs-registry";

import {
  linkDisplayLabel,
  openBlockedLink,
  openExternalLink,
  resolveDocLink,
} from "./docs-links";

// R2-20 跨文跳转回调经 context 下发：components 映射保持模块级单例，
// docs-view 传入的回调随渲染更新（react-markdown 重渲染即生效）。
const DocNavContext = createContext<(docId: string) => void>(() => {});

function flattenText(node: ReactNode): string {
  if (node === null || node === undefined || typeof node === "boolean") return "";
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(flattenText).join("");
  return "";
}

function MarkdownLink({ children, href }: ComponentPropsWithoutRef<"a">) {
  const onOpenDoc = useContext(DocNavContext);
  const anchorText = flattenText(children);
  const link = resolveDocLink(href, PROTOCOL_DOCS);
  const label = link ? linkDisplayLabel(link, anchorText) : anchorText;
  return (
    <a
      href={href}
      title={href ?? undefined}
      className="text-primary underline underline-offset-2 break-words"
      data-doc-link={link?.kind ?? "none"}
      onClick={(event) => {
        if (!link) {
          event.preventDefault();
          console.warn("[docs] 文档链接无落地目标: " + (href ?? "<empty>"));
          return;
        }
        event.preventDefault();
        if (link.kind === "doc") onOpenDoc(link.docId);
        else if (link.kind === "external") void openExternalLink(link.href);
        else void openBlockedLink(link);
      }}
    >
      {label}
    </a>
  );
}

function MarkdownHeading(Tag: "h1" | "h2" | "h3" | "h4") {
  const sizes = {
    h1: "text-lg mt-4 mb-2 first:mt-0",
    h2: "text-base mt-4 mb-1.5",
    h3: "text-sm mt-3 mb-1",
    h4: "text-sm mt-2 mb-1",
  } as const;
  return function MarkdownHeadingOfLevel({
    children,
  }: ComponentPropsWithoutRef<"h4">) {
    return <Tag className={sizes[Tag] + " font-semibold"}>{children}</Tag>;
  };
}

function MarkdownP(props: ComponentPropsWithoutRef<"p">) {
  return (
    <p {...props} className="text-sm leading-relaxed break-words my-1.5" />
  );
}

function MarkdownLi(props: ComponentPropsWithoutRef<"li">) {
  return <li {...props} className="text-sm leading-relaxed break-words" />;
}

// 代码块：等宽 + muted 底 + 横向滚动，色板走主题 token 明暗自适应。
function MarkdownPre(props: ComponentPropsWithoutRef<"pre">) {
  return (
    <pre
      {...props}
      className="bg-muted rounded-md p-3 my-2 overflow-x-auto font-mono text-xs leading-relaxed"
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
      className="border-l-2 border-border pl-3 my-2 text-muted-foreground"
    />
  );
}

function MarkdownTable(props: ComponentPropsWithoutRef<"table">) {
  return (
    <div className="overflow-x-auto my-2">
      <table
        {...props}
        className="w-full border-collapse text-xs break-words"
      />
    </div>
  );
}

function MarkdownTh(props: ComponentPropsWithoutRef<"th">) {
  return (
    <th
      {...props}
      className="border border-border bg-muted/50 px-2 py-1 text-left font-semibold"
    />
  );
}

function MarkdownTd(props: ComponentPropsWithoutRef<"td">) {
  return <td {...props} className="border border-border px-2 py-1 align-top" />;
}

// 协议文档排版映射：GFM（表格/删除线/任务列表）经 remark-gfm；react-markdown
// 默认不渲染原始 HTML（无 rehype-raw），文档内 HTML 片段退化为纯文本。
const COMPONENTS: Components = {
  h1: MarkdownHeading("h1"),
  h2: MarkdownHeading("h2"),
  h3: MarkdownHeading("h3"),
  h4: MarkdownHeading("h4"),
  p: MarkdownP,
  ul: (props) => <ul {...props} className="list-disc pl-5 my-1.5 space-y-0.5" />,
  ol: (props) => (
    <ol {...props} className="list-decimal pl-5 my-1.5 space-y-0.5" />
  ),
  li: MarkdownLi,
  pre: MarkdownPre,
  code: MarkdownCode,
  blockquote: MarkdownBlockquote,
  table: MarkdownTable,
  th: MarkdownTh,
  td: MarkdownTd,
  a: MarkdownLink,
  hr: (props) => <hr {...props} className="border-border my-3" />,
};

// 协议文档 markdown 渲染块：单篇全文渲染，滚动由容器（docs-view 右栏）承担。
// onOpenDoc：站内跨文链接的落地回调（docs-view 切目录+回顶）。
export function DocsMarkdown({
  source,
  onOpenDoc,
}: {
  source: string;
  onOpenDoc: (docId: string) => void;
}) {
  return (
    <DocNavContext.Provider value={onOpenDoc}>
      <div className="text-sm">
        <ReactMarkdown remarkPlugins={[remarkGfm]} components={COMPONENTS}>
          {source}
        </ReactMarkdown>
      </div>
    </DocNavContext.Provider>
  );
}
