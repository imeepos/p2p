import { ChevronDownIcon, ChevronRightIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { HOP_KEY } from "@/components/monitor/hop-timeline";
import { Badge } from "@/components/ui/badge";
import type { Locale } from "@/i18n";
import { formatTime } from "@/lib/format";
import type { NodeEventJson } from "@/lib/ipc-types";
import { cn } from "@/lib/utils";
import { eventTimeMs } from "./event-clock";
import {
  EVENT_TYPE_KEY,
  eventBadgeVariant,
  eventSummary,
  isNodeEventError,
} from "./event-meta";
import { toLooseT } from "./loose-t";
import { EventRowDetail } from "./event-row-detail";

export const EVENT_ROW_HEIGHT = 40;
export const EVENT_ROW_EXPANDED_HEIGHT = 240;

interface EventRowProps {
  event: NodeEventJson;
  locale: Locale;
  expanded: boolean;
  onToggle: (event: NodeEventJson) => void;
}

// 事件行：等宽时间戳 + 类型徽标 + i18n 摘要；整行点击展开详情。
export function EventRow({
  event,
  locale,
  expanded,
  onToggle,
}: EventRowProps) {
  const { t } = useTranslation();
  const tt = toLooseT(t);
  const summary = eventSummary(event, {
    hopLabel: (kind) => tt(HOP_KEY[kind]),
    okLabel: tt("events.outcome.ok"),
    failLabel: tt("events.outcome.fail"),
  });
  const Chevron = expanded ? ChevronDownIcon : ChevronRightIcon;

  return (
    <div className="flex h-full flex-col">
      <button
        type="button"
        onClick={() => onToggle(event)}
        aria-expanded={expanded}
        className="flex h-10 w-full shrink-0 items-center gap-2 border-b px-4 text-left font-mono text-xs hover:bg-muted/40"
      >
        <Chevron
          className="text-muted-foreground size-3.5 shrink-0"
          aria-hidden
        />
        <span className="text-muted-foreground w-20 shrink-0 tabular-nums">
          {formatTime(eventTimeMs(event), locale)}
        </span>
        <Badge variant={eventBadgeVariant(event)} className="shrink-0">
          {t(EVENT_TYPE_KEY[event.type])}
        </Badge>
        <span
          className={cn(
            "min-w-0 flex-1 truncate",
            isNodeEventError(event) && "text-destructive",
          )}
        >
          {tt(summary.key, summary.values)}
        </span>
      </button>
      {expanded && <EventRowDetail event={event} locale={locale} />}
    </div>
  );
}
