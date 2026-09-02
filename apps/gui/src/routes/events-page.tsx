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
import { describeNodeEvent } from "@/lib/event-text";
import { useNodeStore } from "@/stores/node-store";

const SHOWN_EVENT_COUNT = 15;

export function EventsPage() {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const events = useNodeStore((s) => s.events);
  const subscriptionLive = useNodeStore((s) => s.subscriptionLive);
  const shown = events.slice(0, SHOWN_EVENT_COUNT);

  return (
    <>
      <PageHeader titleKey="events.title" descriptionKey="events.description" />
      <StatCard
        label={t("common.labels.events")}
        value={formatNumber(events.length, locale)}
      />
      <StatCard
        label="node-event"
        value={subscriptionLive ? t("common.state.connected") : t("common.state.disconnected")}
      />
      <StatCard label={t("common.labels.version")} value={"v" + __APP_VERSION__} />
      <div className="col-span-12">
        <Card>
          <CardHeader>
            <CardTitle>{t("events.title")}</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-1 font-mono text-xs">
            {shown.length === 0 ? (
              <span className="text-muted-foreground">{t("events.empty")}</span>
            ) : (
              shown.map((event, index) => (
                <div key={index}>{describeNodeEvent(event)}</div>
              ))
            )}
          </CardContent>
        </Card>
      </div>
    </>
  );
}
