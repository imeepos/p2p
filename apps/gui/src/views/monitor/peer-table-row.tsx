import { useTranslation } from "react-i18next";

import { CopyButton } from "@/components/monitor/copy-button";
import { Badge } from "@/components/ui/badge";
import { TableCell, TableRow } from "@/components/ui/table";
import type { Locale } from "@/i18n";
import type { I18nKey } from "@/i18n/types";
import type { PingOutcome } from "@/lib/ipc-types";
import { cn } from "@/lib/utils";
import type { PeerEntry } from "@/stores/node-store";
import { formatRelative } from "./event-clock";
import {
  peerSourceKind,
  peerStatusKind,
  type PeerSourceKind,
  type PeerStatusKind,
} from "./peer-status";
import { PeerRowActions } from "./peer-row-actions";

const SHORT_PEER_ID_LEN = 10;

const STATUS_DOT: Record<PeerStatusKind, string> = {
  connected: "bg-emerald-500",
  discovered: "bg-sky-500",
  offline: "bg-muted-foreground/40",
};

const STATUS_KEY: Record<PeerStatusKind, I18nKey> = {
  connected: "common.state.connected",
  discovered: "common.state.discovered",
  offline: "common.state.offline",
};

const SOURCE_KEY: Record<PeerSourceKind, I18nKey> = {
  manual: "peers.source.manual",
  discovered: "peers.source.discovered",
};

const SOURCE_HINT_KEY: Record<PeerSourceKind, I18nKey> = {
  manual: "peers.source.manualHint",
  discovered: "peers.source.discoveredHint",
};

interface PeerTableRowProps {
  peer: PeerEntry;
  locale: Locale;
  now: number;
  onPing: (peer: PeerEntry) => () => Promise<PingOutcome>;
  onShowDetail: (peerId: string) => void;
}

export function PeerTableRow({
  peer,
  locale,
  now,
  onPing,
  onShowDetail,
}: PeerTableRowProps) {
  const { t } = useTranslation();
  const status = peerStatusKind(peer, now);
  const source = peerSourceKind(peer);

  return (
    <TableRow>
      <TableCell className="font-mono text-xs">
        <span className="flex items-center gap-1">
          <span title={peer.peerId}>
            {peer.peerId.slice(0, SHORT_PEER_ID_LEN)}…
          </span>
          <CopyButton value={peer.peerId} className="size-6" />
        </span>
      </TableCell>
      <TableCell className="max-w-56 font-mono text-xs">
        <span className="block truncate" title={peer.addrs.join(" | ")}>
          {peer.addrs[0] ?? "-"}
          {peer.addrs.length > 1 && ` +${peer.addrs.length - 1}`}
        </span>
      </TableCell>
      <TableCell>
        <Badge
          variant={source === "manual" ? "secondary" : "outline"}
          title={t(SOURCE_HINT_KEY[source])}
        >
          {t(SOURCE_KEY[source])}
        </Badge>
      </TableCell>
      <TableCell>
        <span className="flex items-center gap-1.5 text-xs">
          <span
            className={cn("size-2 rounded-full", STATUS_DOT[status])}
            aria-hidden
          />
          {t(STATUS_KEY[status])}
        </span>
      </TableCell>
      <TableCell className="text-xs tabular-nums">
        {peer.lastSeenMs > 0
          ? formatRelative(peer.lastSeenMs, locale, now)
          : "-"}
      </TableCell>
      <TableCell>
        <PeerRowActions
          peer={peer}
          onPing={onPing}
          onShowDetail={onShowDetail}
        />
      </TableCell>
    </TableRow>
  );
}
