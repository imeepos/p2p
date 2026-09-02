import { useTranslation } from "react-i18next";

import type { Locale } from "@/i18n";
import type { NodeEventJson } from "@/lib/ipc-types";
import { eventTimeMs } from "./event-clock";

interface EventRowDetailProps {
  event: NodeEventJson;
  locale: Locale;
}

// 展开区：接收时间（含日期）+ 原始负载 JSON。
export function EventRowDetail({ event, locale }: EventRowDetailProps) {
  const { t } = useTranslation();
  const at = eventTimeMs(event);

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 border-b bg-muted/40 px-4 py-2 text-xs">
      <div className="flex gap-2">
        <span className="text-muted-foreground shrink-0">
          {t("events.detail.receivedAt")}
        </span>
        <span className="font-mono">
          {new Intl.DateTimeFormat(locale, {
            dateStyle: "short",
            timeStyle: "medium",
          }).format(at)}
        </span>
      </div>
      <div className="flex min-h-0 gap-2">
        <span className="text-muted-foreground shrink-0">
          {t("events.detail.payload")}
        </span>
        <pre className="min-w-0 flex-1 overflow-auto font-mono break-all whitespace-pre-wrap">
          {JSON.stringify(event, null, 2)}
        </pre>
      </div>
    </div>
  );
}
