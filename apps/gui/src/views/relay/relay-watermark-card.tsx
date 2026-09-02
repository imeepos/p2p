import { useTranslation } from "react-i18next";

import { StatCard } from "@/components/page/stat-card";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import type { Locale } from "@/i18n";
import { formatNumber } from "@/lib/format";
import { useNodeStore } from "@/stores/node-store";

// 水位卡：中继会话与重连计数，实时读 store metrics。
export function RelayWatermarkCard() {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const metrics = useNodeStore((s) => s.metrics);

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("relay.watermark.title")}</CardTitle>
        <CardDescription>{t("relay.watermark.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="grid grid-cols-2 gap-4">
        {metrics === null ? (
          <>
            <Skeleton className="h-12" />
            <Skeleton className="h-12" />
          </>
        ) : (
          <>
            <StatCard
              label={t("relay.watermark.sessions")}
              value={formatNumber(metrics.relaySessionsActive, locale)}
            />
            <StatCard
              label={t("relay.watermark.reconnects")}
              value={formatNumber(metrics.relayReconnects, locale)}
            />
          </>
        )}
      </CardContent>
    </Card>
  );
}
