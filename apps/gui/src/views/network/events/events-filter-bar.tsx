import { RotateCcwIcon, SearchIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import {
  ALL_EVENT_TYPES,
  EVENT_TYPE_KEY,
} from "@/views/network/event-meta";
import type { NodeEventType } from "@/lib/ipc-types";
import {
  EVENT_FILTER_GROUPS,
  type EventFilterGroup,
} from "./event-filter-groups";

interface EventsFilterBarProps {
  query: string;
  onQueryChange: (query: string) => void;
  errorOnly: boolean;
  onErrorOnlyChange: (value: boolean) => void;
  typeFilter: ReadonlySet<NodeEventType>;
  onToggleType: (type: NodeEventType) => void;
  /** 按类型统计的缓冲区命中数（空缓冲全 0），与筛选状态无关。 */
  counts: Record<NodeEventType, number>;
  onResetFilters: () => void;
}

function isFiltersActive(
  query: string,
  errorOnly: boolean,
  typeFilter: ReadonlySet<NodeEventType>,
): boolean {
  return query.trim().length > 0 || errorOnly || typeFilter.size !== ALL_EVENT_TYPES.length;
}

function FilterGroupRow({
  group,
  typeFilter,
  counts,
  onToggleType,
}: {
  group: EventFilterGroup;
  typeFilter: ReadonlySet<NodeEventType>;
  counts: Record<NodeEventType, number>;
  onToggleType: (type: NodeEventType) => void;
}) {
  const { t } = useTranslation();
  return (
    <div
      className="flex items-start gap-2"
      data-testid={`event-filter-group-${group.id}`}
    >
      <span className="text-muted-foreground w-14 shrink-0 pt-1 text-xs">
        {t(group.labelKey)}
      </span>
      <div className="flex flex-wrap gap-1.5">
        {group.types.map((type) => {
          const on = typeFilter.has(type);
          const count = counts[type] ?? 0;
          const label = t(EVENT_TYPE_KEY[type]);
          return (
            <Button
              key={type}
              size="sm"
              variant={on ? "secondary" : "outline"}
              className="h-7 px-2.5 text-xs"
              aria-pressed={on}
              aria-label={t("uxiEvents.filter.chipAria", { label, count })}
              data-testid={`event-filter-chip-${type}`}
              onClick={() => onToggleType(type)}
            >
              {label}
              <span className="text-muted-foreground tabular-nums">
                {count}
              </span>
            </Button>
          );
        })}
      </div>
    </div>
  );
}

// 过滤器：文本搜索、仅错误开关、类型多选按域分组（F20），chip 附命中计数，
// 尾部「清除筛选」一键恢复默认（任一筛选生效时可用）。
export function EventsFilterBar({
  query,
  onQueryChange,
  errorOnly,
  onErrorOnlyChange,
  typeFilter,
  onToggleType,
  counts,
  onResetFilters,
}: EventsFilterBarProps) {
  const { t } = useTranslation();
  const active = isFiltersActive(query, errorOnly, typeFilter);

  return (
    <>
      <div className="col-span-12 flex flex-wrap items-center gap-2">
        <div className="relative">
          <SearchIcon
            className="text-muted-foreground absolute top-1/2 left-2.5 size-4 -translate-y-1/2"
            aria-hidden
          />
          <Input
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder={t("events.filter.searchPlaceholder")}
            className="h-9 w-56 pl-8"
          />
        </div>
        <label className="flex items-center gap-2 text-sm">
          <Switch
            checked={errorOnly}
            onCheckedChange={onErrorOnlyChange}
            aria-label={t("events.filter.errorOnly")}
          />
          {t("events.filter.errorOnly")}
        </label>
      </div>
      <div className="col-span-12 flex flex-col gap-1.5">
        {EVENT_FILTER_GROUPS.map((group) => (
          <FilterGroupRow
            key={group.id}
            group={group}
            typeFilter={typeFilter}
            counts={counts}
            onToggleType={onToggleType}
          />
        ))}
        <div className="flex justify-end">
          <Button
            size="sm"
            variant="ghost"
            className="h-7 text-xs"
            disabled={!active}
            onClick={onResetFilters}
            data-testid="event-filter-clear"
          >
            <RotateCcwIcon aria-hidden />
            {t("events.filter.reset")}
          </Button>
        </div>
      </div>
    </>
  );
}
