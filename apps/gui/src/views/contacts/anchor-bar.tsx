import { useTranslation } from "react-i18next";

import { cn } from "@/lib/utils";

import { CONTACTS_SECTIONS, type ContactsSectionId } from "./contacts-sections";

interface AnchorBarProps {
  active: ContactsSectionId;
  onGo: (id: ContactsSectionId) => void;
}

// 页顶锚点条（§3.1）：好友 | 群 | Agent，点击滚动定位，当前节高亮。
export function AnchorBar({ active, onGo }: AnchorBarProps) {
  const { t } = useTranslation();
  return (
    <nav
      aria-label={t("contacts.title")}
      data-testid="contacts-anchor-bar"
      className="bg-card ring-border ring-1 flex items-center gap-1 rounded-lg p-1"
    >
      {CONTACTS_SECTIONS.map((id) => {
        const label = t(`contacts.section.${id}`);
        return (
          <button
            key={id}
            type="button"
            onClick={() => onGo(id)}
            aria-label={t("contacts.anchor.goto", { section: label })}
            aria-current={active === id ? "true" : undefined}
            data-active={active === id}
            data-testid={`contacts-anchor-${id}`}
            className={cn(
              "hover:bg-accent rounded-md px-3 py-1.5 text-sm",
              active === id && "bg-accent font-semibold",
            )}
          >
            {label}
          </button>
        );
      })}
    </nav>
  );
}
