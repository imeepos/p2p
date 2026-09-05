import { useTranslation } from "react-i18next";

import { PageHeader } from "@/components/page/page-header";

// /contacts 占位页（分期表 P0 行）：先锚定路由与命令面板三个区锚点的
// 目标位置，三区布局与添加流归 P2（docs/design/app-shell-redesign.md 三）。
const SECTIONS = ["friends", "groups", "agents"] as const;

export function ContactsPage() {
  const { t } = useTranslation();
  return (
    <div className="col-span-12 flex flex-col gap-4">
      <PageHeader
        titleKey="contacts.title"
        descriptionKey="contacts.placeholder.description"
      />
      {SECTIONS.map((section) => (
        <section
          key={section}
          id={section}
          aria-label={t(`contacts.section.${section}`)}
          className="bg-card ring-border ring-1 rounded-lg p-4"
        >
          <h2 className="text-sm font-semibold">{t(`contacts.section.${section}`)}</h2>
          <p className="text-muted-foreground mt-1 text-sm">
            {t("contacts.placeholder.description")}
          </p>
        </section>
      ))}
    </div>
  );
}
