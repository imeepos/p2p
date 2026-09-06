import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import type { Locale } from "@/i18n";
import type { NodeEventJson } from "@/lib/ipc-types";
import { RecentEventLine } from "./recent-event-line";
import { useTicker } from "@/views/network/use-ticker";

// 4.2 第 4 块：最近事件只读摘要固定 5 条；标题栏「查看全部」跳事件 tab。
const RECENT_EVENT_COUNT = 5;

interface OverviewRecentEventsCardProps {
  events: NodeEventJson[];
  loading: boolean;
  /** 订阅引导失败：给显式错误文案，不永挂骨架。 */
  linkFailed?: boolean;
}

export function OverviewRecentEventsCard({
  events,
  loading,
  linkFailed = false,
}: OverviewRecentEventsCardProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const now = useTicker(1000);
  const recent = events.slice(0, RECENT_EVENT_COUNT);

  return (
    <div className="col-span-12 lg:col-span-6">
      <Card className="flex h-full min-h-56 flex-col gap-3 py-4">
        <CardHeader className="flex-row items-center justify-between space-y-0 px-4">
          <CardTitle className="text-base">
            {t("dashboard.cards.recentEvents")}
          </CardTitle>
          <Link
            to="/network/events"
            className="-m-2 p-2 text-muted-foreground hover:text-foreground text-xs transition-colors"
          >
            {t("network.overview.viewAll")}
          </Link>
        </CardHeader>
        <CardContent className="flex flex-1 flex-col justify-center gap-1.5 px-4">
          {linkFailed ? (
            <p className="text-destructive text-sm" role="alert">
              {t("events.loadFailed")}
            </p>
          ) : loading ? (
            <div className="flex flex-col gap-2">
              <Skeleton className="h-4 w-full" />
              <Skeleton className="h-4 w-4/5" />
              <Skeleton className="h-4 w-3/5" />
            </div>
          ) : recent.length === 0 ? (
            <p className="text-muted-foreground text-sm">
              {t("dashboard.events.empty")}
            </p>
          ) : (
            recent.map((event, index) => (
              <RecentEventLine
                key={index}
                event={event}
                locale={locale}
                now={now}
              />
            ))
          )}
        </CardContent>
      </Card>
    </div>
  );
}
