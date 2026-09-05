import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";

import { NETWORK_TABS } from "../network-tab-memory";

// 4.2 第 5 块：排障入口行——三链接直达节点/中继/诊断 tab；rail 收敛后
// 低频排障面的兜底发现路径（设计文档七、风险表对策落点）。
const TROUBLESHOOT_TAB_IDS = ["peers", "relay", "diagnostics"] as const;

export function TroubleshootLinks() {
  const { t } = useTranslation();

  return (
    <div className="col-span-12 flex flex-wrap items-center gap-2">
      <span className="text-muted-foreground text-sm">
        {t("network.overview.troubleshootTitle")}
      </span>
      {TROUBLESHOOT_TAB_IDS.map((id) => {
        const tab = NETWORK_TABS.find((entry) => entry.id === id)!;
        return (
          <Link
            key={id}
            to={`/network/${tab.path}`}
            className="rounded-md border px-3 py-1.5 text-sm transition-colors hover:bg-accent hover:text-accent-foreground"
          >
            {t(tab.labelKey)}
          </Link>
        );
      })}
    </div>
  );
}
