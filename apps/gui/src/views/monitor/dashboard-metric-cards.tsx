import { useTranslation } from "react-i18next";

import { StatCard } from "@/components/page/stat-card";
import type { Locale } from "@/i18n";
import { formatNumber } from "@/lib/format";
import type { MetricsJson } from "@/lib/ipc-types";
import { useNodeStore, selectPeerCount } from "@/stores/node-store";

interface DashboardMetricCardsProps {
  metrics: MetricsJson | null;
}

// 指标卡 x4：已发现节点数、活跃连接、中继会话、门禁拒绝。
export function DashboardMetricCards({ metrics }: DashboardMetricCardsProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const peerCount = useNodeStore(selectPeerCount);

  return (
    <>
      <StatCard
        span={3}
        label={t("dashboard.cards.peers")}
        // 与其余指标卡同口径：首次取数未到一律骨架，避免把「还没数据」
        // 渲染成 0 被误读为「没有节点」。
        loading={metrics === null}
        value={
          metrics === null ? undefined : formatNumber(peerCount, locale)
        }
      />
      <StatCard
        span={3}
        label={t("dashboard.cards.connections")}
        loading={metrics === null}
        value={
          metrics ? formatNumber(metrics.activeConnections, locale) : undefined
        }
      />
      <StatCard
        span={3}
        label={t("dashboard.cards.relaySessions")}
        loading={metrics === null}
        value={
          metrics
            ? formatNumber(metrics.relaySessionsActive, locale)
            : undefined
        }
      />
      <StatCard
        span={3}
        label={t("dashboard.cards.gateDenials")}
        loading={metrics === null}
        value={
          metrics ? formatNumber(metrics.gateDenialsTotal, locale) : undefined
        }
      />
    </>
  );
}
