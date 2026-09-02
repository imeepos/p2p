import { useTranslation } from "react-i18next";

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

interface HopRow {
  key: "punch" | "relay";
  ok: number;
  fail: number;
}

// 逐跳统计卡：dial_punch / dial_relay 的 ok/fail 横向比例条。
export function HopStatsCard() {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const metrics = useNodeStore((s) => s.metrics);

  const rows: HopRow[] = metrics
    ? [
        { key: "punch", ok: metrics.dialPunchOk, fail: metrics.dialPunchFail },
        { key: "relay", ok: metrics.dialRelayOk, fail: metrics.dialRelayFail },
      ]
    : [];

  return (
    <Card className="col-span-12 lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("relay.hops.title")}</CardTitle>
        <CardDescription>{t("relay.hops.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {metrics === null ? (
          <>
            <Skeleton className="h-10" />
            <Skeleton className="h-10" />
          </>
        ) : (
          rows.map((row) => (
            <div key={row.key} className="flex flex-col gap-1.5">
              <div className="flex items-center justify-between text-xs">
                <span className="font-medium">
                  {t(row.key === "punch" ? "relay.hops.punch" : "relay.hops.relay")}
                </span>
                {row.ok + row.fail === 0 ? (
                  <span className="text-muted-foreground">
                    {t("relay.hops.empty")}
                  </span>
                ) : (
                  <span className="text-muted-foreground">
                    {formatNumber(row.ok, locale)} {t("relay.hops.ok")} /{" "}
                    {formatNumber(row.fail, locale)} {t("relay.hops.fail")}
                  </span>
                )}
              </div>
              <div className="bg-muted motion-safe:animate-in motion-safe:fade-in flex h-2.5 w-full overflow-hidden rounded-full">
                <div
                  className="bg-success transition-all motion-reduce:transition-none"
                  style={{ width: (row.ok / Math.max(1, row.ok + row.fail)) * 100 + "%" }}
                />
                <div
                  className="bg-destructive transition-all motion-reduce:transition-none"
                  style={{ width: (row.fail / Math.max(1, row.ok + row.fail)) * 100 + "%" }}
                />
              </div>
            </div>
          ))
        )}
      </CardContent>
    </Card>
  );
}