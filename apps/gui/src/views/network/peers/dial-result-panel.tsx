import { useTranslation } from "react-i18next";

import { HopTimeline } from "@/components/monitor/hop-timeline";
import { Badge } from "@/components/ui/badge";
import type { DialReport } from "@/lib/ipc-types";

interface DialResultPanelProps {
  report: DialReport;
}

// 拨号结果：成败徽标 + 耗时 + 逐跳时间线（direct/punch/relay）。
export function DialResultPanel({ report }: DialResultPanelProps) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-3 rounded-md border p-3">
      <div className="flex items-center justify-between gap-2">
        <span className="text-sm font-medium">{t("peers.dial.result")}</span>
        <span className="flex items-center gap-2">
          <span className="text-muted-foreground font-mono text-xs">
            {t("peers.dial.elapsed", { ms: report.totalMs })}
          </span>
          <Badge variant={report.ok ? "default" : "destructive"}>
            {report.ok ? t("peers.dial.succeeded") : t("peers.dial.failed")}
          </Badge>
        </span>
      </div>
      <HopTimeline hops={report.hops} />
    </div>
  );
}
