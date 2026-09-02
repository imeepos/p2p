import { useTranslation } from "react-i18next";

import {
  Sparkline,
  type SparklineTone,
} from "@/components/charts/sparkline";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { MetricsPoint } from "@/lib/ipc-types";
import type { I18nKey } from "@/i18n/types";

interface TrendSeriesProps {
  points: MetricsPoint[];
  valueKey: "activeConnections" | "relaySessionsActive";
  labelKey: I18nKey;
  tone: SparklineTone;
}

function TrendSeries({ points, valueKey, labelKey, tone }: TrendSeriesProps) {
  const { t } = useTranslation();
  const sparkPoints = points.map((point) => ({
    tMs: point.tMs,
    value: point[valueKey],
  }));

  return (
    <div className="flex flex-col gap-1">
      <span className="text-muted-foreground text-xs">{t(labelKey)}</span>
      <Sparkline points={sparkPoints} tone={tone} label={t(labelKey)} />
    </div>
  );
}

interface DashboardTrendCardProps {
  history: MetricsPoint[];
  running: boolean;
}

// 10 分钟趋势卡：活跃连接 / 中继会话双 sparkline；未运行显示引导空态。
export function DashboardTrendCard({ history, running }: DashboardTrendCardProps) {
  const { t } = useTranslation();

  return (
    <div className="col-span-12">
      <Card className="h-full gap-3 py-4">
        <CardHeader className="px-4">
          <CardTitle className="text-base">{t("dashboard.trend.title")}</CardTitle>
          <p className="text-muted-foreground text-xs">
            {t("dashboard.trend.hint")}
          </p>
        </CardHeader>
        <CardContent className="px-4">
          {history.length === 0 ? (
            <div
              className="text-muted-foreground flex flex-col items-center gap-1 rounded-md border border-dashed py-8"
              role="status"
            >
              <span className="text-sm">{t("dashboard.trend.empty")}</span>
              <span className="text-xs">
                {t(
                  running
                    ? "dashboard.trend.hint"
                    : "dashboard.trend.emptyHint",
                )}
              </span>
            </div>
          ) : (
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <TrendSeries
                points={history}
                valueKey="activeConnections"
                labelKey="dashboard.trend.connections"
                tone="success"
              />
              <TrendSeries
                points={history}
                valueKey="relaySessionsActive"
                labelKey="dashboard.trend.relaySessions"
                tone="warning"
              />
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
