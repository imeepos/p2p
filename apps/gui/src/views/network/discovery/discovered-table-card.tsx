import { CopyIcon, RadarIcon } from "lucide-react";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
import { EmptyState } from "@/views/shared/empty-state";
import { PeerIdCell } from "@/views/shared/peer-id-cell";

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
    <Table containerClassName="max-h-80 overflow-y-auto">
      <TableHeader className="[&_th]:sticky [&_th]:top-0 [&_th]:z-10 [&_th]:bg-card">
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
            <TableCell className="max-w-48 font-mono text-xs">
              <PeerIdCell peerId={peer.peerId} />
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

interface DiscoveredTableCardProps {
  /** mDNS 已启用（含未保存草稿开启）：空态的「开启 mDNS」入口随之禁用 */
  mdnsActive: boolean;
  onEnableMdns: () => void;
  onAddAddress: () => void;
}

// 发现结果表：数据完全由 store 的 node-event 派生（peer_discovered/connected）。
// 空态文案承诺「开启 mDNS 或添加引导地址」——两个入口按钮就地兑现承诺。
export function DiscoveredTableCard({
  mdnsActive,
  onEnableMdns,
  onAddAddress,
}: DiscoveredTableCardProps) {
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
          <EmptyState
            icon={RadarIcon}
            title={t("discovery.empty")}
            description={t("discovery.emptyHint")}
            action={
              <div className="flex flex-wrap justify-center gap-2">
                <Button
                  type="button"
                  size="sm"
                  disabled={mdnsActive}
                  onClick={onEnableMdns}
                >
                  {t("discovery.emptyActions.enableMdns")}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={onAddAddress}
                >
                  {t("discovery.emptyActions.addBootstrap")}
                </Button>
              </div>
            }
          />
        ) : (
          <PeerTable peers={peers} firstSeen={firstSeen} />
        )}
      </CardContent>
    </Card>
  );
}