import { useTranslation } from "react-i18next";
import { useState } from "react";
import { useSearchParams } from "react-router-dom";

import { PageHeader } from "@/components/page/page-header";
import type { Locale } from "@/i18n";
import { selectPeerList, useNodeStore } from "@/stores/node-store";
import { PeerDialDialog } from "./peer-dial-dialog";
import { PeerDetailSheet } from "./peer-detail-sheet";
import { PeersTableCard } from "./peers-table-card";
import { PeersToolbar, type StatusFilter } from "./peers-toolbar";
import { peerStatusKind } from "./peer-status";
import { useTicker } from "./use-ticker";

const PING_TIMEOUT_MS = 8000;

function matchesSearch(
  peer: { peerId: string; addrs: string[] },
  query: string,
): boolean {
  const q = query.trim().toLowerCase();
  if (q.length === 0) return true;
  return (
    peer.peerId.toLowerCase().includes(q) ||
    peer.addrs.some((addr) => addr.toLowerCase().includes(q))
  );
}

export function PeersView() {
  const { i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const peers = useNodeStore(selectPeerList);
  const ping = useNodeStore((s) => s.ping);
  const [searchParams, setSearchParams] = useSearchParams();
  const [query, setQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [detailId, setDetailId] = useState<string | null>(null);
  const now = useTicker(5000);

  const dialOpen = searchParams.get("dial") === "1";
  const setDialOpen = (open: boolean) => {
    setSearchParams(open ? { dial: "1" } : {});
  };

  const filtered = peers.filter(
    (peer) =>
      matchesSearch(peer, query) &&
      (statusFilter === "all" || peerStatusKind(peer, now) === statusFilter),
  );
  const detailPeer = peers.find((peer) => peer.peerId === detailId) ?? null;
  const onPing = (peer: { peerId: string }) => () =>
    ping(peer.peerId, PING_TIMEOUT_MS);

  return (
    <>
      <PageHeader titleKey="peers.title" descriptionKey="peers.description" />

      <PeersToolbar
        query={query}
        onQueryChange={setQuery}
        statusFilter={statusFilter}
        onStatusFilterChange={setStatusFilter}
        onOpenDial={() => setDialOpen(true)}
      />

      <PeersTableCard
        peers={filtered}
        bufferEmpty={peers.length === 0}
        locale={locale}
        now={now}
        onPing={onPing}
        onShowDetail={setDetailId}
      />

      <PeerDialDialog open={dialOpen} onOpenChange={setDialOpen} />
      <PeerDetailSheet
        peer={detailPeer}
        onOpenChange={(open) => {
          if (!open) setDetailId(null);
        }}
      />
    </>
  );
}
