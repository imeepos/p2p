import { useState } from "react";
import { BookOpen } from "lucide-react";
import { useTranslation } from "react-i18next";

import { PageHeader } from "@/components/page/page-header";
import { PROTOCOL_DOCS } from "@/config/docs-registry";
import { EmptyState } from "@/views/shared/empty-state";

import { DocsMarkdown } from "./docs-markdown";

// /docs 协议文档页（DOC2 双栏）：左侧五篇目录切换，右侧内容滚动渲染。
// 正文单源自仓库 docs/protocol/ raw 引入（config/docs-registry.ts），
// 页面 chrome 走 zh/en i18n；目录标题为文档自身 H1（正文不翻译）。
export function DocsView() {
  const { t } = useTranslation();
  const [activeId, setActiveId] = useState(PROTOCOL_DOCS[0].id);
  const active =
    PROTOCOL_DOCS.find((doc) => doc.id === activeId) ?? PROTOCOL_DOCS[0];

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
        <section
          aria-label={active.title}
          className="min-w-0 flex-1 overflow-y-auto rounded-lg border bg-card p-4"
        >
          {active.markdown.trim().length > 0 ? (
            <DocsMarkdown source={active.markdown} />
          ) : (
            <EmptyState icon={BookOpen} title={t("docs.empty")} />
          )}
        </section>
      </div>
    </>
  );
}
