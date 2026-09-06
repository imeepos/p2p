import { ChevronDownIcon, ChevronRightIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { HOP_KEY } from "@/views/network/hop-labels";
import { Badge } from "@/components/ui/badge";
import type { Locale } from "@/i18n";
import { formatTime } from "@/lib/format";
import type { NodeEventJson } from "@/lib/ipc-types";
import { usePeerNameLabel } from "@/lib/peer-name";
import { cn } from "@/lib/utils";
import { eventTimeMs } from "@/views/network/event-clock";
import {
  EVENT_TYPE_KEY,
  eventBadgeVariant,
  eventSummary,
  isNodeEventError,
} from "@/views/network/event-meta";
import { toLooseT } from "@/views/network/loose-t";
import { EventRowDetail } from "./event-row-detail";

export const EVENT_ROW_HEIGHT = 40;
export const EVENT_ROW_EXPANDED_HEIGHT = 240;

interface EventRowProps {
  event: NodeEventJson;
  locale: Locale;
  expanded: boolean;
  onToggle: (event: NodeEventJson) => void;
}

// 事件行（F16）：行主体点击与行尾「详情」按钮都能展开；展开态容器挂
// data-state=open 并整体高亮（底色 + 主色内嵌条），收起/再展开状态由
// 上层 Set 驱动，行为一致。
export function EventRow({
  event,
  locale,
  expanded,
  onToggle,
}: EventRowProps) {
  const { t } = useTranslation();
  const tt = toLooseT(t);
  const peerLabel = usePeerNameLabel();
  const summary = eventSummary(
    event,
    {
      hopLabel: (kind) => tt(HOP_KEY[kind]),
      okLabel: tt("events.outcome.ok"),
      failLabel: tt("events.outcome.fail"),
    },
    { peerLabel },
  );
  const Chevron = expanded ? ChevronDownIcon : ChevronRightIcon;

  return (
    <div
      data-state={expanded ? "open" : "closed"}
      className={cn(
        "flex h-full flex-col",
        expanded &&
          "bg-muted/50 shadow-[inset_2px_0_0_0_var(--primary)]",
      )}
    >
      <div className="flex h-10 shrink-0 items-stretch border-b">
        <button
          type="button"
          onClick={() => onToggle(event)}
          aria-expanded={expanded}
          className="flex min-w-0 flex-1 items-center gap-2 px-4 text-left font-mono text-xs hover:bg-muted/40"
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
        <button
          type="button"
          onClick={() => onToggle(event)}
          aria-expanded={expanded}
          data-testid="event-row-details"
          className="text-muted-foreground hover:bg-muted/40 hover:text-foreground shrink-0 border-l px-3 text-xs"
        >
          {t("uxiEvents.row.details")}
        </button>
      </div>
      {expanded && <EventRowDetail event={event} locale={locale} />}
    </div>
  );
}
