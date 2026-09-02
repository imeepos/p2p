import { useTranslation } from "react-i18next";

import { FeedbackDemoCard } from "@/components/feedback/feedback-demo-card";
import { PageHeader } from "@/components/page/page-header";
import { StatCard } from "@/components/page/stat-card";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { Locale } from "@/i18n";
import { describeNodeEvent } from "@/lib/event-text";
import { formatNumber, formatUptime } from "@/lib/format";
import { useNodeStore, selectPeerCount } from "@/stores/node-store";

const RECENT_EVENT_COUNT = 5;

export function DashboardPage() {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const status = useNodeStore((s) => s.status);
  const metrics = useNodeStore((s) => s.metrics);
  const events = useNodeStore((s) => s.events);
  const peerCount = useNodeStore(selectPeerCount);

  const loading = status === null;
  const recent = events.slice(0, RECENT_EVENT_COUNT);

  return (
    <>
      <PageHeader
        titleKey="dashboard.title"
        descriptionKey="dashboard.description"
      />
      <StatCard
        label={t("dashboard.cards.status")}
        loading={loading}
        value={
          status
            ? status.running
              ? t("common.state.running")
              : t("common.state.stopped")
            : undefined
        }
      />
      <StatCard
        label={t("dashboard.cards.peerId")}
        loading={loading}
        mono
        value={status?.peerId ?? t("common.state.unknown")}
      />
      <StatCard
        label={t("dashboard.cards.uptime")}
        loading={loading}
        value={
          status ? formatUptime(status.uptimeSecs, locale) : undefined
        }
      />
      <StatCard
        label={t("dashboard.cards.listenAddrs")}
        loading={loading}
        mono
        value={status?.listenAddrs[0] ?? t("common.labels.none")}
      />
      <StatCard
        label={t("dashboard.cards.peers")}
        value={formatNumber(peerCount, locale)}
        loading={loading}
      />
      <StatCard
        label={t("dashboard.cards.connections")}
        value={
          metrics ? formatNumber(metrics.activeConnections, locale) : undefined
        }
        loading={metrics === null}
      />
      <StatCard
        label={t("dashboard.cards.relaySessions")}
        value={
          metrics
            ? formatNumber(metrics.relaySessionsActive, locale)
            : undefined
        }
        loading={metrics === null}
      />
      <StatCard
        label={t("dashboard.cards.gateDenials")}
        value={
          metrics
            ? formatNumber(metrics.gateDenialsTotal, locale)
            : undefined
        }
        loading={metrics === null}
      />
      <FeedbackDemoCard />
      <div className="col-span-12">
        <Card>
          <CardHeader>
            <CardTitle>{t("dashboard.cards.recentEvents")}</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-col gap-1 font-mono text-xs">
            {recent.length === 0 ? (
              <span className="text-muted-foreground">
                {t("dashboard.events.empty")}
              </span>
            ) : (
              recent.map((event, index) => (
                <div key={index}>{describeNodeEvent(event)}</div>
              ))
            )}
          </CardContent>
        </Card>
      </div>
    </>
  );
}
