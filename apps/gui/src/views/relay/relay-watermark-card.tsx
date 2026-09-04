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

function WatermarkValue({
  label,
  value,
  locale,
}: {
  label: string;
  value: number;
  locale: Locale;
}) {
  // IM-V2 R2：分立嵌套框（边框小卡）+ 缩小字号，与左列表单卡节奏对齐。
  return (
    <div className="min-w-0 rounded-md border p-3">
      <span className="text-muted-foreground block text-xs">{label}</span>
      <span className="mt-1 block text-lg font-semibold tabular-nums">
        {formatNumber(value, locale)}
      </span>
    </div>
  );
}

// 水位卡：中继会话与重连计数，实时读 store metrics。
// 数字层级（IM-V2 R2）：嵌套边框小卡 + text-lg semibold，与配置卡同高同节奏。
export function RelayWatermarkCard() {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const metrics = useNodeStore((s) => s.metrics);

  return (
    <Card className="col-span-12 h-full lg:col-span-6">
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
            <WatermarkValue
              label={t("relay.watermark.sessions")}
              value={metrics.relaySessionsActive}
              locale={locale}
            />
            <WatermarkValue
              label={t("relay.watermark.reconnects")}
              value={metrics.relayReconnects}
              locale={locale}
            />
          </>
        )}
      </CardContent>
    </Card>
  );
}
