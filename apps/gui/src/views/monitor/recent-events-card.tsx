import { useTranslation } from "react-i18next";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import type { Locale } from "@/i18n";
import type { NodeEventJson } from "@/lib/ipc-types";
import { RecentEventLine } from "./recent-event-line";
import { useTicker } from "./use-ticker";

const RECENT_EVENT_COUNT = 10;

interface RecentEventsCardProps {
  events: NodeEventJson[];
  loading: boolean;
  /** 订阅引导失败：给显式错误文案，不永挂骨架。 */
  linkFailed?: boolean;
}

// 最近事件卡：最新 10 条，类型徽标 + 相对时间（1s 跳动）。
export function RecentEventsCard({
  events,
  loading,
  linkFailed = false,
}: RecentEventsCardProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const now = useTicker(1000);
  const recent = events.slice(0, RECENT_EVENT_COUNT);

  return (
    <div className="col-span-12 lg:col-span-6">
      <Card className="flex h-full min-h-56 flex-col gap-3 py-4">
        <CardHeader className="px-4">
          <CardTitle className="text-base">
            {t("dashboard.cards.recentEvents")}
          </CardTitle>
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
