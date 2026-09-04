import { SearchIcon } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { I18nKey } from "@/i18n/types";
import type { PeerStatusKind } from "./peer-status";

export type StatusFilter = "all" | PeerStatusKind;

const FILTER_LABEL: Record<StatusFilter, I18nKey> = {
  all: "common.all",
  connected: "common.state.connected",
  discovered: "common.state.discovered",
  offline: "common.state.offline",
};

interface PeersToolbarProps {
  query: string;
  onQueryChange: (query: string) => void;
  statusFilter: StatusFilter;
  onStatusFilterChange: (filter: StatusFilter) => void;
  onOpenDial: () => void;
}

// 工具栏：搜索框 + 状态过滤 tabs + 手动拨号入口。
export function PeersToolbar({
  query,
  onQueryChange,
  statusFilter,
  onStatusFilterChange,
  onOpenDial,
}: PeersToolbarProps) {
  const { t } = useTranslation();
  const filters: StatusFilter[] = [
    "all",
    "connected",
    "discovered",
    "offline",
  ];

  return (
    <div className="col-span-12 flex flex-wrap items-center gap-2">
      <div className="relative w-56">
        <SearchIcon
          className="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2"
          aria-hidden
        />
        <Input
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          placeholder={t("peers.searchPlaceholder")}
          className="h-9 pl-8"
        />
      </div>
      <Tabs
        value={statusFilter}
        onValueChange={(value) => onStatusFilterChange(value as StatusFilter)}
      >
        <TabsList>
          {filters.map((filter) => (
            <TabsTrigger
              key={filter}
              value={filter}
              // IM-V2 P2：选中项实心填充 + 未选中弱化（覆盖 ui/tabs 底态）
              className="text-muted-foreground data-[state=active]:border-primary data-[state=active]:bg-primary data-[state=active]:text-primary-foreground dark:data-[state=active]:border-primary dark:data-[state=active]:bg-primary dark:data-[state=active]:text-primary-foreground"
            >
              {t(FILTER_LABEL[filter])}
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>
      <div className="flex-1" />
      <Button size="sm" onClick={onOpenDial}>
        {t("peers.dial.title")}
      </Button>
    </div>
  );
}
