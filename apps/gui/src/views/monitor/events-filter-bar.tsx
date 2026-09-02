import { SearchIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import {
  ALL_EVENT_TYPES,
  EVENT_TYPE_KEY,
} from "./event-meta";
import type { NodeEventType } from "@/lib/ipc-types";

interface EventsFilterBarProps {
  query: string;
  onQueryChange: (query: string) => void;
  errorOnly: boolean;
  onErrorOnlyChange: (value: boolean) => void;
  typeFilter: ReadonlySet<NodeEventType>;
  onToggleType: (type: NodeEventType) => void;
}

// 过滤器：文本搜索、仅错误开关、类型多选（按下态表示包含）。
export function EventsFilterBar({
  query,
  onQueryChange,
  errorOnly,
  onErrorOnlyChange,
  typeFilter,
  onToggleType,
}: EventsFilterBarProps) {
  const { t } = useTranslation();

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
      <div className="col-span-12 flex flex-wrap items-center gap-1.5">
        {ALL_EVENT_TYPES.map((type) => {
          const on = typeFilter.has(type);
          return (
            <Button
              key={type}
              size="sm"
              variant={on ? "secondary" : "outline"}
              className="h-7 px-2.5 text-xs"
              aria-pressed={on}
              onClick={() => onToggleType(type)}
            >
              {t(EVENT_TYPE_KEY[type])}
            </Button>
          );
        })}
      </div>
    </>
  );
}
