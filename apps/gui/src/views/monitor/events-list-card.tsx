import { useTranslation } from "react-i18next";

import { VirtualList } from "@/components/monitor/virtual-list";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import type { Locale } from "@/i18n";
import type { NodeEventJson } from "@/lib/ipc-types";
import { EventRow } from "./event-row";

interface EventsListCardProps {
  loading: boolean;
  bufferEmpty: boolean;
  filtered: NodeEventJson[];
  locale: Locale;
  expanded: ReadonlySet<NodeEventJson>;
  heightAt: (event: NodeEventJson) => number;
  onToggle: (event: NodeEventJson) => void;
}

// 事件流卡片：加载骨架 / 空态 / 虚拟滚动列表。
export function EventsListCard({
  loading,
  bufferEmpty,
  filtered,
  locale,
  expanded,
  heightAt,
  onToggle,
}: EventsListCardProps) {
  const { t } = useTranslation();

  return (
    <div className="col-span-12">
      <Card className="gap-0 py-0">
        <CardContent className="p-0">
          {loading ? (
            <div className="flex flex-col gap-2 p-4">
              <Skeleton className="h-6 w-full" />
              <Skeleton className="h-6 w-full" />
              <Skeleton className="h-6 w-4/5" />
            </div>
          ) : filtered.length === 0 ? (
            <p className="text-muted-foreground p-6 text-sm">
              {bufferEmpty ? t("events.empty") : t("events.emptyFiltered")}
            </p>
          ) : (
            <VirtualList
              className="h-[60vh] min-h-80"
              items={filtered}
              heightAt={heightAt}
              renderItem={(event) => (
                <EventRow
                  event={event}
                  locale={locale}
                  expanded={expanded.has(event)}
                  onToggle={onToggle}
                />
              )}
            />
          )}
        </CardContent>
      </Card>
    </div>
  );
}
