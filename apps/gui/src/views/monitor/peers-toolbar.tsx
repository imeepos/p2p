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
      <Input
        value={query}
        onChange={(event) => onQueryChange(event.target.value)}
        placeholder={t("peers.searchPlaceholder")}
        className="h-9 w-56"
      />
      <Tabs
        value={statusFilter}
        onValueChange={(value) => onStatusFilterChange(value as StatusFilter)}
      >
        <TabsList>
          {filters.map((filter) => (
            <TabsTrigger key={filter} value={filter}>
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
