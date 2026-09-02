import {
  ArrowDownIcon,
  ArrowUpDownIcon,
  ArrowUpIcon,
  SearchXIcon,
  UsersIcon,
} from "lucide-react";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { Locale } from "@/i18n";
import type { PingOutcome } from "@/lib/ipc-types";
import type { PeerEntry } from "@/stores/node-store";
import { cn } from "@/lib/utils";
import { EmptyState } from "@/views/shared/empty-state";
import { peerStatusKind, type PeerStatusKind } from "./peer-status";
import { PeerTableRow } from "./peer-table-row";

type SortKey = "lastSeen" | "status";
type SortDir = "asc" | "desc";

interface SortState {
  key: SortKey;
  dir: SortDir;
}

const STATUS_RANK: Record<PeerStatusKind, number> = {
  connected: 0,
  discovered: 1,
  offline: 2,
};

function comparePeers(
  a: PeerEntry,
  b: PeerEntry,
  key: SortKey,
  dir: SortDir,
  now: number,
): number {
  const sign = dir === "asc" ? 1 : -1;
  if (key === "status") {
    return (
      sign * (STATUS_RANK[peerStatusKind(a, now)] - STATUS_RANK[peerStatusKind(b, now)])
    );
  }
  return sign * (a.lastSeenMs - b.lastSeenMs);
}

function SortableHead({
  label,
  sortKey,
  sort,
  onSort,
}: {
  label: string;
  sortKey: SortKey;
  sort: SortState;
  onSort: (key: SortKey) => void;
}) {
  const active = sort.key === sortKey;
  const Icon = !active
    ? ArrowUpDownIcon
    : sort.dir === "asc"
      ? ArrowUpIcon
      : ArrowDownIcon;

  return (
    <TableHead
      aria-sort={active ? (sort.dir === "asc" ? "ascending" : "descending") : "none"}
    >
      <button
        type="button"
        className={cn(
          "hover:text-foreground flex items-center gap-1 transition-colors",
          active && "text-foreground",
        )}
        onClick={() => onSort(sortKey)}
      >
        {label}
        <Icon className="size-3" aria-hidden />
      </button>
    </TableHead>
  );
}

interface PeersTableCardProps {
  peers: PeerEntry[];
  bufferEmpty: boolean;
  locale: Locale;
  now: number;
  onPing: (peer: PeerEntry) => () => Promise<PingOutcome>;
  onShowDetail: (peerId: string) => void;
  onOpenDial: () => void;
}

// 节点主表卡片：可排序（状态/最后活跃）、粘性表头、空态区分缓冲与过滤。
export function PeersTableCard({
  peers,
  bufferEmpty,
  locale,
  now,
  onPing,
  onShowDetail,
  onOpenDial,
}: PeersTableCardProps) {
  const { t } = useTranslation();
  const [sort, setSort] = useState<SortState>({ key: "lastSeen", dir: "desc" });

  const sorted = useMemo(
    () => [...peers].sort((a, b) => comparePeers(a, b, sort.key, sort.dir, now)),
    [peers, sort, now],
  );

  const toggleSort = (key: SortKey) =>
    setSort((prev) =>
      prev.key === key
        ? { key, dir: prev.dir === "asc" ? "desc" : "asc" }
        : { key, dir: key === "status" ? "asc" : "desc" },
    );

  return (
    <div className="col-span-12">
      <Card className="gap-3 py-4">
        <CardContent className="px-0">
          {peers.length === 0 ? (
            <div className="px-6">
              <EmptyState
                icon={bufferEmpty ? UsersIcon : SearchXIcon}
                title={
                  bufferEmpty ? t("peers.empty") : t("common.table.empty")
                }
                description={
                  bufferEmpty ? t("peers.emptyHint") : undefined
                }
                action={
                  bufferEmpty ? (
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      onClick={onOpenDial}
                    >
                      {t("peers.emptyAction")}
                    </Button>
                  ) : undefined
                }
              />
            </div>
          ) : (
            <Table containerClassName="max-h-96 overflow-y-auto">
              <TableHeader className="[&_th]:sticky [&_th]:top-0 [&_th]:z-10 [&_th]:bg-card">
                <TableRow>
                  <TableHead>{t("common.labels.peerId")}</TableHead>
                  <TableHead>{t("common.labels.address")}</TableHead>
                  <TableHead>{t("common.labels.source")}</TableHead>
                  <SortableHead
                    label={t("common.labels.status")}
                    sortKey="status"
                    sort={sort}
                    onSort={toggleSort}
                  />
                  <SortableHead
                    label={t("common.labels.lastSeen")}
                    sortKey="lastSeen"
                    sort={sort}
                    onSort={toggleSort}
                  />
                  <TableHead className="w-44" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {sorted.map((peer) => (
                  <PeerTableRow
                    key={peer.peerId}
                    peer={peer}
                    locale={locale}
                    now={now}
                    onPing={onPing}
                    onShowDetail={onShowDetail}
                  />
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
