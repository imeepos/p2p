import { useTranslation } from "react-i18next";

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
import { PeerTableRow } from "./peer-table-row";

interface PeersTableCardProps {
  peers: PeerEntry[];
  bufferEmpty: boolean;
  locale: Locale;
  now: number;
  onPing: (peer: PeerEntry) => () => Promise<PingOutcome>;
  onShowDetail: (peerId: string) => void;
}

// 节点主表卡片：空态区分「缓冲为空」与「过滤后无匹配」。
export function PeersTableCard({
  peers,
  bufferEmpty,
  locale,
  now,
  onPing,
  onShowDetail,
}: PeersTableCardProps) {
  const { t } = useTranslation();

  return (
    <div className="col-span-12">
      <Card className="gap-3 py-4">
        <CardContent className="px-0">
          {peers.length === 0 ? (
            <p className="text-muted-foreground px-6 text-sm">
              {bufferEmpty ? t("peers.empty") : t("common.table.empty")}
            </p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("common.labels.peerId")}</TableHead>
                  <TableHead>{t("common.labels.address")}</TableHead>
                  <TableHead>{t("common.labels.source")}</TableHead>
                  <TableHead>{t("common.labels.status")}</TableHead>
                  <TableHead>{t("common.labels.lastSeen")}</TableHead>
                  <TableHead className="w-44" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {peers.map((peer) => (
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
