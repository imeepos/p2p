import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";

import type { Locale } from "@/i18n";
import { formatNumber } from "@/lib/format";
import { useNodeStore, selectListenPorts } from "@/stores/node-store";

export function StatusBar() {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const running = useNodeStore((s) => s.status?.running ?? false);
  const ports = useNodeStore(useShallow((s) => selectListenPorts(s.status)));
  const connections = useNodeStore((s) => s.metrics?.activeConnections ?? 0);

  return (
    <footer className="text-muted-foreground flex h-8 shrink-0 items-center gap-4 border-t px-4 text-xs">
      <span className="flex items-center gap-1.5">
        <span
          className={
            running ? "size-2 rounded-full bg-success" : "size-2 rounded-full bg-muted-foreground/40"
          }
          aria-hidden
        />
        {running ? t("common.state.running") : t("common.state.stopped")}
      </span>
      {ports.length > 0 && (
        <span>
          {t("common.labels.ports")}: {ports.join(", ")}
        </span>
      )}
      <span>
        {t("common.labels.activeConnections")}: {formatNumber(connections, locale)}
      </span>
      <span className="ml-auto">
        {t("common.labels.version")}: v{__APP_VERSION__}
      </span>
    </footer>
  );
}
