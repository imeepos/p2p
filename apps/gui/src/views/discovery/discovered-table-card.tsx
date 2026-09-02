import { CopyIcon } from "lucide-react";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { Locale } from "@/i18n";
import { formatTime } from "@/lib/format";
import type { NodeEventJson } from "@/lib/ipc-types";
import { selectPeerList, useNodeStore, type PeerEntry } from "@/stores/node-store";
import { copyText } from "@/views/shared/clipboard";

// 契约 v1 修订：事件可携带可选 tsMs；缺省时兜底用 store 记录的本地接收时间。
function readEventTsMs(event: NodeEventJson): number | null {
  const tsMs = (event as { tsMs?: number }).tsMs;
  return typeof tsMs === "number" && Number.isFinite(tsMs) ? tsMs : null;
}

// events 新事件在前；从最旧端扫起，首次命中即该节点的最早发现时刻。
function deriveFirstSeen(events: NodeEventJson[]): Map<string, number | null> {
  const firstSeen = new Map<string, number | null>();
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index];
    if (event.type !== "peer_discovered" || firstSeen.has(event.peer)) continue;
    firstSeen.set(event.peer, readEventTsMs(event));
  }
  return firstSeen;
}

function PeerTable({
  peers,
  firstSeen,
}: {
  peers: PeerEntry[];
  firstSeen: Map<string, number | null>;
}) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{t("common.labels.peerId")}</TableHead>
          <TableHead>{t("common.labels.address")}</TableHead>
          <TableHead>{t("discovery.table.firstSeen")}</TableHead>
          <TableHead className="w-12" aria-label={t("common.actions.copy")} />
        </TableRow>
      </TableHeader>
      <TableBody>
        {peers.map((peer) => (
          <TableRow key={peer.peerId}>
            <TableCell
              className="max-w-48 truncate font-mono text-xs"
              title={peer.peerId}
            >
              {peer.peerId.slice(0, 16)}
            </TableCell>
            <TableCell className="font-mono text-xs">
              {peer.addrs.join(", ") || t("common.labels.none")}
            </TableCell>
            <TableCell className="text-xs">
              {formatTime(firstSeen.get(peer.peerId) ?? peer.lastSeenMs, locale)}
            </TableCell>
            <TableCell>
              <Button
                variant="ghost"
                size="icon"
                aria-label={t("common.actions.copy")}
                onClick={() => {
                  void copyText(peer.peerId, {
                    done: t("settings.identity.copyDone"),
                    failed: t("settings.identity.copyFailed"),
                  });
                }}
              >
                <CopyIcon aria-hidden />
              </Button>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

// 发现结果表：数据完全由 store 的 node-event 派生（peer_discovered/connected）。
export function DiscoveredTableCard() {
  const { t } = useTranslation();
  const peers = useNodeStore(selectPeerList);
  const events = useNodeStore((s) => s.events);
  const firstSeen = useMemo(() => deriveFirstSeen(events), [events]);

  return (
    <Card className="col-span-12">
      <CardHeader>
        <CardTitle>{t("discovery.table.title")}</CardTitle>
        <CardDescription>{t("discovery.table.hint")}</CardDescription>
      </CardHeader>
      <CardContent>
        {peers.length === 0 ? (
          <p className="text-muted-foreground text-sm">{t("discovery.empty")}</p>
        ) : (
          <PeerTable peers={peers} firstSeen={firstSeen} />
        )}
      </CardContent>
    </Card>
  );
}
