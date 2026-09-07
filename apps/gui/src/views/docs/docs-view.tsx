import { useCallback, useRef, useState } from "react";
import { ArrowUp, BookOpen } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { PageHeader } from "@/components/page/page-header";
import { PROTOCOL_DOCS } from "@/config/docs-registry";
import { EmptyState } from "@/views/shared/empty-state";

import { DocsMarkdown } from "./docs-markdown";

// R2-21 长文页内导航：滚动超过约一屏出「返回顶部」浮动按钮
const BACK_TOP_THRESHOLD_PX = 240;

// /docs 协议文档页（DOC2 双栏）：左侧五篇目录切换，右侧内容滚动渲染。
// 正文单源自仓库 docs/protocol/ raw 引入（config/docs-registry.ts），
// 页面 chrome 走 zh/en i18n；目录标题为文档自身 H1（正文不翻译）。
// R2-20：文内跨篇链接切换目录并回顶；不可达路径给反馈+复制（docs-links）。
export function DocsView() {
  const { t } = useTranslation();
  const [activeId, setActiveId] = useState(PROTOCOL_DOCS[0].id);
  const [showBackTop, setShowBackTop] = useState(false);
  const active =
    PROTOCOL_DOCS.find((doc) => doc.id === activeId) ?? PROTOCOL_DOCS[0];

  const contentRef = useRef<HTMLElement | null>(null);
  const openDoc = useCallback((docId: string) => {
    setActiveId(docId);
    setShowBackTop(false);
    // 跨文落地从目标篇开头读，沿用目录切换的回顶语义
    // （scrollTop 直写：jsdom 无 Element.scrollTo，行为等价）
    if (contentRef.current) contentRef.current.scrollTop = 0;
  }, []);

  const onContentScroll = useCallback(() => {
    setShowBackTop((contentRef.current?.scrollTop ?? 0) > BACK_TOP_THRESHOLD_PX);
  }, []);

  const backToTop = useCallback(() => {
    if (contentRef.current) contentRef.current.scrollTop = 0;
    setShowBackTop(false);
  }, []);

  return (
    <>
      <PageHeader titleKey="docs.title" descriptionKey="docs.description" />
      <div className="flex min-h-0 flex-1 gap-4">
        <nav
          aria-label={t("docs.toc")}
          className="w-56 shrink-0 overflow-y-auto rounded-lg border p-2"
        >
          <p className="text-muted-foreground px-2 pb-1 text-xs font-medium">
            {t("docs.toc")}
          </p>
          <ul className="flex flex-col gap-0.5">
            {PROTOCOL_DOCS.map((doc) => (
              <li key={doc.id}>
                <button
                  type="button"
                  aria-current={doc.id === active.id ? "page" : undefined}
                  className={
                    "w-full rounded-md px-2 py-1.5 text-left text-sm transition-colors " +
                    (doc.id === active.id
                      ? "bg-muted font-medium text-foreground"
                      : "text-muted-foreground hover:bg-muted/60 hover:text-foreground")
                  }
                  onClick={() => setActiveId(doc.id)}
                >
                  {doc.title}
                </button>
              </li>
            ))}
          </ul>
        </nav>
        <div className="relative min-h-0 flex-1">
          <section
            ref={contentRef}
            aria-label={active.title}
            onScroll={onContentScroll}
            className="h-full min-w-0 overflow-y-auto rounded-lg border bg-card p-4"
          >
            {active.markdown.trim().length > 0 ? (
              <DocsMarkdown source={active.markdown} onOpenDoc={openDoc} />
            ) : (
              <EmptyState icon={BookOpen} title={t("docs.empty")} />
            )}
          </section>
          {showBackTop ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              aria-label={t("docs.backToTop")}
              data-testid="docs-back-top"
              onClick={backToTop}
              className="absolute bottom-4 right-4"
            >
              <ArrowUp aria-hidden className="size-4" />
            </Button>
          ) : null}
        </div>
      </div>
    </>
  );
}
