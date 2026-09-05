import { useTranslation } from "react-i18next";
import { useCallback, useState } from "react";
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
  const connect = useNodeStore((s) => s.connect);
  const disconnect = useNodeStore((s) => s.disconnect);
  const status = useNodeStore((s) => s.status);
  const startNode = useNodeStore((s) => s.startNode);
  const [searchParams, setSearchParams] = useSearchParams();
  const [query, setQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [detailId, setDetailId] = useState<string | null>(null);
  const now = useTicker(5000);

  // 空态按节点运行状态分支：未运行给启动引导，而不是只给注定失败的拨号入口。
  const nodeReady = status !== null;
  const nodeRunning = status?.running ?? false;
  const onStartNode = useCallback(async () => {
    const current = useNodeStore.getState().status;
    if (!current) throw new Error("node status not loaded");
    await startNode(current.config);
  }, [startNode]);
  const resetFilters = useCallback(() => {
    setQuery("");
    setStatusFilter("all");
  }, []);

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
  const onConnect = (peer: { peerId: string }) => () => connect(peer.peerId);
  const onDisconnect = (peer: { peerId: string }) => () =>
    disconnect(peer.peerId);

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
        nodeReady={nodeReady}
        nodeRunning={nodeRunning}
        onStartNode={onStartNode}
        onResetFilters={resetFilters}
        locale={locale}
        now={now}
        onPing={onPing}
        onConnect={onConnect}
        onDisconnect={onDisconnect}
        onShowDetail={setDetailId}
        onOpenDial={() => setDialOpen(true)}
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
