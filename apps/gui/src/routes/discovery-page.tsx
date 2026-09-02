import { useTranslation } from "react-i18next";

import { PageHeader } from "@/components/page/page-header";
import { StatCard } from "@/components/page/stat-card";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { Locale } from "@/i18n";
import { formatNumber } from "@/lib/format";
import { useNodeStore, selectPeerCount } from "@/stores/node-store";

const DISCOVERED_SHOWN = 8;

export function DiscoveryPage() {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const peerCount = useNodeStore(selectPeerCount);
  const discovered = useNodeStore((s) =>
    s.events.filter((event) => event.type === "peer_discovered"),
  );
  const shown = discovered.slice(0, DISCOVERED_SHOWN);

  return (
    <>
      <PageHeader
        titleKey="discovery.title"
        descriptionKey="discovery.description"
      />
      <StatCard
        label={t("dashboard.cards.peers")}
        value={formatNumber(peerCount, locale)}
      />
      <StatCard
        label={t("discovery.title")}
        value={formatNumber(discovered.length, locale)}
      />
      <StatCard label={t("common.labels.status")} value="-" />
      <StatCard label={t("common.labels.version")} value={"v" + __APP_VERSION__} />
      <div className="col-span-12">
        <Card>
          <CardHeader>
            <CardTitle>{t("discovery.title")}</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-1 font-mono text-xs">
            {shown.length === 0 ? (
              <span className="text-muted-foreground">
                {t("discovery.empty")}
              </span>
            ) : (
              shown.map((event, index) =>
                event.type === "peer_discovered" ? (
                  <div key={index}>
                    {event.peer.slice(0, 12)} {"-> "}
                    {event.addrs.join(", ")}
                  </div>
                ) : null,
              )
            )}
          </CardContent>
        </Card>
      </div>
    </>
  );
}
