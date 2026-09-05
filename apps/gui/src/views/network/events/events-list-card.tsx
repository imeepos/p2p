import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { VirtualList } from "@/components/monitor/virtual-list";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import type { Locale } from "@/i18n";
import type { NodeEventJson } from "@/lib/ipc-types";
import { EventRow } from "./event-row";

export interface EventsListCardProps {
  loading: boolean;
  linkFailed: boolean;
  onRetryLink: () => Promise<void>;
  bufferEmpty: boolean;
  filtered: NodeEventJson[];
  onResetFilters: () => void;
  locale: Locale;
  expanded: ReadonlySet<NodeEventJson>;
  heightAt: (event: NodeEventJson) => number;
  onToggle: (event: NodeEventJson) => void;
}

// 事件流卡片：链接错误态 / 加载骨架 / 空态 / 虚拟滚动列表。
export function EventsListCard({
  loading,
  linkFailed,
  onRetryLink,
  bufferEmpty,
  filtered,
  onResetFilters,
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
          {linkFailed ? (
            <div className="flex flex-col items-start gap-2 p-4">
              <p className="text-destructive text-sm" role="alert">
                {t("events.loadFailed")}
              </p>
              <AsyncButton
                size="sm"
                variant="outline"
                action={onRetryLink}
                loadingLabel={t("dashboard.dataLink.retrying")}
              >
                {t("dashboard.dataLink.retry")}
              </AsyncButton>
            </div>
          ) : loading ? (
            <div className="flex flex-col gap-2 p-4">
              <Skeleton className="h-6 w-full" />
              <Skeleton className="h-6 w-full" />
              <Skeleton className="h-6 w-4/5" />
            </div>
          ) : filtered.length === 0 ? (
            bufferEmpty ? (
              <p className="text-muted-foreground p-6 text-sm">
                {t("events.empty")}
              </p>
            ) : (
              <div className="flex flex-col items-start gap-2 p-6">
                <p className="text-muted-foreground text-sm">
                  {t("events.emptyFiltered")}
                </p>
                <Button size="sm" variant="outline" onClick={onResetFilters}>
                  {t("events.filter.reset")}
                </Button>
              </div>
            )
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
