import { useTranslation } from "react-i18next";

import { PageHeader } from "@/components/page/page-header";
import { StatCard } from "@/components/page/stat-card";
import type { Locale } from "@/i18n";
import { formatNumber } from "@/lib/format";
import { useNodeStore } from "@/stores/node-store";

export function RelayPage() {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const metrics = useNodeStore((s) => s.metrics);

  return (
    <>
      <PageHeader titleKey="relay.title" descriptionKey="relay.description" />
      <StatCard
        label={t("dashboard.cards.relaySessions")}
        value={
          metrics ? formatNumber(metrics.relaySessionsActive, locale) : undefined
        }
        loading={metrics === null}
      />
      <StatCard
        label="relayReconnects"
        value={
          metrics ? formatNumber(metrics.relayReconnects, locale) : undefined
        }
        loading={metrics === null}
      />
      <StatCard
        label="dialRelayOk"
        value={metrics ? formatNumber(metrics.dialRelayOk, locale) : undefined}
        loading={metrics === null}
      />
      <StatCard
        label="dialRelayFail"
        value={metrics ? formatNumber(metrics.dialRelayFail, locale) : undefined}
        loading={metrics === null}
      />
      <div className="col-span-12">
        <p className="text-muted-foreground text-sm">{t("relay.empty")}</p>
      </div>
    </>
  );
}
