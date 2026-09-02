import { useTranslation } from "react-i18next";

import { PageHeader } from "@/components/page/page-header";
import { StatCard } from "@/components/page/stat-card";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
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
import { formatNumber, formatTime } from "@/lib/format";
import { useNodeStore, selectPeerList } from "@/stores/node-store";

export function PeersPage() {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const peers = useNodeStore(selectPeerList);
  const connectedCount = peers.filter((peer) => peer.connected).length;

  return (
    <>
      <PageHeader titleKey="peers.title" descriptionKey="peers.description" />
      <StatCard
        label={t("dashboard.cards.peers")}
        value={formatNumber(peers.length, locale)}
      />
      <StatCard
        label={t("common.state.connected")}
        value={formatNumber(connectedCount, locale)}
      />
      <StatCard
        label={t("common.labels.events")}
        value="-"
      />
      <StatCard
        label={t("common.labels.version")}
        value={"v" + __APP_VERSION__}
      />
      <div className="col-span-12">
        <Card>
          <CardHeader>
            <CardTitle>{t("peers.count", { count: peers.length })}</CardTitle>
          </CardHeader>
          <CardContent className="px-0">
            {peers.length === 0 ? (
              <p className="text-muted-foreground px-6 text-sm">
                {t("peers.empty")}
              </p>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>{t("common.labels.peerId")}</TableHead>
                    <TableHead>{t("common.labels.address")}</TableHead>
                    <TableHead>{t("common.labels.status")}</TableHead>
                    <TableHead>{t("common.labels.lastSeen")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {peers.map((peer) => (
                    <TableRow key={peer.peerId}>
                      <TableCell className="font-mono text-xs">
                        {peer.peerId.slice(0, 12)}
                      </TableCell>
                      <TableCell className="font-mono text-xs">
                        {peer.addrs[0] ?? "-"}
                      </TableCell>
                      <TableCell>
                        <Badge variant={peer.connected ? "default" : "outline"}>
                          {peer.connected
                            ? t("common.state.connected")
                            : t("common.state.disconnected")}
                        </Badge>
                      </TableCell>
                      <TableCell>
                        {peer.lastSeenMs > 0
                          ? formatTime(peer.lastSeenMs, locale)
                          : "-"}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </CardContent>
        </Card>
      </div>
    </>
  );
}
