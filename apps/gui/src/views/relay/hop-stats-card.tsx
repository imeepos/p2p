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
// 行样式（IM-V1 R4）：h-10 紧凑行 + divide-y 分隔，空态灰斜体。
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

  // ok/fail 两段按占比并排（flex 横排），fail>0 且 ok=0 时 fail 段满宽可见。
  const segmentWidth = (count: number, total: number): string =>
    total === 0 ? "0%" : ((count / total) * 100).toFixed(4) + "%";

  return (
    <Card className="col-span-12 h-full lg:col-span-6">
      <CardHeader>
        <CardTitle>{t("relay.hops.title")}</CardTitle>
        <CardDescription>{t("relay.hops.hint")}</CardDescription>
      </CardHeader>
      <CardContent>
        {metrics === null ? (
          <div className="flex flex-col gap-2">
            <Skeleton className="h-10" />
            <Skeleton className="h-10" />
          </div>
        ) : (
          <div className="divide-y">
            {rows.map((row) => (
              <div
                key={row.key}
                className="flex h-10 items-center gap-3 text-xs"
              >
                <span className="shrink-0 font-medium">
                  {t(
                    row.key === "punch"
                      ? "relay.hops.punch"
                      : "relay.hops.relay",
                  )}
                </span>
                <div
                  className="bg-muted motion-safe:animate-in motion-safe:fade-in h-2 flex-1 overflow-hidden rounded-full"
                  role="img"
                  aria-label={`${t(
                    row.key === "punch"
                      ? "relay.hops.punch"
                      : "relay.hops.relay",
                  )} ${formatNumber(row.ok, locale)} / ${formatNumber(
                    row.ok + row.fail,
                    locale,
                  )}`}
                >
                  <div className="flex h-full w-full">
                    <div
                      data-testid={`hop-${row.key}-ok`}
                      className="bg-success h-full transition-all motion-reduce:transition-none"
                      style={{
                        width: segmentWidth(row.ok, row.ok + row.fail),
                      }}
                    />
                    <div
                      data-testid={`hop-${row.key}-fail`}
                      className="bg-destructive h-full transition-all motion-reduce:transition-none"
                      style={{
                        width: segmentWidth(row.fail, row.ok + row.fail),
                      }}
                    />
                  </div>
                </div>
                {row.ok + row.fail === 0 ? (
                  <span className="text-muted-foreground w-28 shrink-0 text-right italic">
                    {t("relay.hops.empty")}
                  </span>
                ) : (
                  <span className="text-muted-foreground w-28 shrink-0 text-right tabular-nums">
                    {formatNumber(row.ok, locale)} {t("relay.hops.ok")} /{" "}
                    {formatNumber(row.fail, locale)} {t("relay.hops.fail")}
                  </span>
                )}
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
