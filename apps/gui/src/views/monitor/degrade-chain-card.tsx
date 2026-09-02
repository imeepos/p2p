import { useTranslation } from "react-i18next";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import type { I18nKey } from "@/i18n/types";
import type { MetricsJson } from "@/lib/ipc-types";

interface ChainRowDef {
  labelKey: I18nKey;
  ok: number;
  fail: number;
}

function chainRows(metrics: MetricsJson | null): ChainRowDef[] {
  if (!metrics) return [];
  return [
    { labelKey: "dashboard.chain.direct", ok: metrics.dialDirectOk, fail: metrics.dialDirectFail },
    { labelKey: "dashboard.chain.punch", ok: metrics.dialPunchOk, fail: metrics.dialPunchFail },
    { labelKey: "dashboard.chain.relay", ok: metrics.dialRelayOk, fail: metrics.dialRelayFail },
  ];
}

function ChainBar({ row }: { row: ChainRowDef }) {
  const { t } = useTranslation();
  const total = row.ok + row.fail;
  const okPct = total > 0 ? Math.round((row.ok * 100) / total) : 0;

  return (
    <div className="flex items-center gap-3 text-xs">
      <span className="text-muted-foreground w-10 shrink-0">
        {t(row.labelKey)}
      </span>
      <div
        className="bg-muted h-2 flex-1 overflow-hidden rounded-full"
        role="img"
        aria-label={`${t(row.labelKey)} ${okPct}%`}
      >
        {total > 0 && (
          <div className="flex h-full">
            <div className="bg-emerald-500 h-full" style={{ width: `${okPct}%` }} />
            <div className="bg-destructive h-full" style={{ width: `${100 - okPct}%` }} />
          </div>
        )}
      </div>
      <span className="text-muted-foreground w-32 shrink-0 text-right tabular-nums">
        {total > 0
          ? `${t("dashboard.chain.okCount", { count: row.ok })} / ${t("dashboard.chain.failCount", { count: row.fail })}`
          : t("dashboard.chain.empty")}
      </span>
    </div>
  );
}

interface DegradeChainCardProps {
  metrics: MetricsJson | null;
  loading: boolean;
}

// 降级链成功率：direct/punch/relay 三行 ok/fail 比例条。
export function DegradeChainCard({ metrics, loading }: DegradeChainCardProps) {
  const { t } = useTranslation();

  return (
    <div className="col-span-12 lg:col-span-6">
      <Card className="h-full gap-3 py-4">
        <CardHeader className="px-4">
          <CardTitle className="text-base">{t("dashboard.chain.title")}</CardTitle>
        </CardHeader>
        <CardContent className="px-4">
          {loading ? (
            <Skeleton className="h-16 w-full" />
          ) : (
            <div className="flex flex-col gap-2.5">
              {chainRows(metrics).map((row) => (
                <ChainBar key={row.labelKey} row={row} />
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
