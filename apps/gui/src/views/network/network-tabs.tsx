import { useTranslation } from "react-i18next";
import { Link, useLocation } from "react-router-dom";

import { cn } from "@/lib/utils";
import { NETWORK_TABS } from "./network-tab-memory";

// tab 条（docs/design/app-shell-redesign.md 4.1）：容器内常驻，选中态由
// 当前子路由派生；role=tablist/tab 供读屏与 DOM 断言定位。
export function NetworkTabBar() {
  const { t } = useTranslation();
  const { pathname } = useLocation();

  return (
    <nav
      aria-label={t("network.title")}
      className="col-span-12 flex flex-wrap items-center gap-1 border-b pb-2"
    >
      {NETWORK_TABS.map((tab) => {
        const to = `/network/${tab.path}`;
        const selected = pathname === to;
        return (
          <Link
            key={tab.id}
            to={to}
            role="tab"
            aria-selected={selected}
            aria-current={selected ? "page" : undefined}
            className={cn(
              "rounded-md px-3 py-1.5 text-sm transition-colors focus-visible:ring-ring/50 focus-visible:ring-[3px] focus-visible:outline-none",
              selected
                ? "bg-accent text-accent-foreground font-medium"
                : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
            )}
          >
            {t(tab.labelKey)}
          </Link>
        );
      })}
    </nav>
  );
}
