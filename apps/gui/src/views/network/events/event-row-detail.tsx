import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import type { Locale } from "@/i18n";
import type { NodeEventJson } from "@/lib/ipc-types";
import { copyText } from "@/views/shared/clipboard";
import { eventTimeMs } from "@/views/network/event-clock";

interface EventRowDetailProps {
  event: NodeEventJson;
  locale: Locale;
}

// 展开区：接收时间（含日期）+ 原始负载 JSON。
// R2-16：原始负载是开发者向信息，默认折叠（对照诊断页 F27「复制详情」
// 口径），需要时手动展开；「复制详情」一键取完整 JSON，失败 toast 可见。
export function EventRowDetail({ event, locale }: EventRowDetailProps) {
  const { t } = useTranslation();
  const [jsonOpen, setJsonOpen] = useState(false);
  const at = eventTimeMs(event);
  const payload = JSON.stringify(event, null, 2);
  const copyDetails = () => {
    void copyText(payload, {
      done: t("events.detail.copied"),
      failed: t("common.copyFailed"),
    });
  };

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
      <div className="flex items-center gap-2">
        <span className="text-muted-foreground shrink-0">
          {t("events.detail.payload")}
        </span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          aria-expanded={jsonOpen}
          data-testid="event-detail-json-toggle"
          onClick={() => setJsonOpen((v) => !v)}
        >
          {t(jsonOpen ? "events.detail.hideJson" : "events.detail.showJson")}
        </Button>
        <Button
          type="button"
          variant="outline"
          size="sm"
          data-testid="event-detail-copy"
          onClick={copyDetails}
        >
          {t("events.detail.copyDetails")}
        </Button>
      </div>
      {jsonOpen && (
        <div className="flex min-h-0 gap-2">
          <pre className="min-w-0 flex-1 overflow-auto font-mono break-all whitespace-pre-wrap">
            {payload}
          </pre>
        </div>
      )}
    </div>
  );
}
