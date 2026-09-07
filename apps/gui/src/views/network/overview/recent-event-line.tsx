import { useTranslation } from "react-i18next";

import { HOP_KEY } from "@/views/network/hop-labels";
import { Badge } from "@/components/ui/badge";
import type { Locale } from "@/i18n";
import type { DialHopKind, NodeEventJson } from "@/lib/ipc-types";
import { eventTimeMs, formatRelative } from "@/views/network/event-clock";
import { EVENT_TYPE_KEY, eventBadgeVariant, eventSummary } from "@/views/network/event-meta";
import { toLooseT } from "@/views/network/loose-t";
import { usePeerNameLabel } from "@/lib/peer-name";

interface RecentEventLineProps {
  event: NodeEventJson;
  locale: Locale;
  now: number;
}

// 最近事件单行：类型徽标 + i18n 摘要 + 相对时间。
// R2-17：peer 标签与事件页同一 usePeerNameLabel 源（好友名 + 前 6…后 4），
// 消灭概览 8 位前缀、事件页 6+4 两种截断并存。
export function RecentEventLine({ event, locale, now }: RecentEventLineProps) {
  const { t } = useTranslation();
  const tt = toLooseT(t);
  const peerLabel = usePeerNameLabel();
  const labels = {
    hopLabel: (kind: DialHopKind) => tt(HOP_KEY[kind]),
    okLabel: tt("events.outcome.ok"),
    failLabel: tt("events.outcome.fail"),
  };
  const summary = eventSummary(event, labels, { peerLabel });
  // tooltip 与行内文案共用同一 i18n 模板（不截断 PeerId），
  // 不再展示 describeNodeEvent 的原始协议串，消灭中英混排。
  const fullSummary = eventSummary(event, labels, { full: true });

  return (
    <div className="flex items-center gap-2 text-xs">
      <Badge variant={eventBadgeVariant(event)} className="shrink-0">
        {t(EVENT_TYPE_KEY[event.type])}
      </Badge>
      <span
        className="min-w-0 flex-1 truncate"
        title={tt(fullSummary.key, fullSummary.values)}
      >
        {tt(summary.key, summary.values)}
      </span>
      <span className="text-muted-foreground shrink-0 tabular-nums">
        {formatRelative(eventTimeMs(event), locale, now)}
      </span>
    </div>
  );
}
