import { useTranslation } from "react-i18next";
import { useCallback, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";

import { PageHeader } from "@/components/page/page-header";
import type { Locale } from "@/i18n";
import { peerKnownName, usePeerNameSource } from "@/lib/peer-name";
import { selectPeerList, useNodeStore } from "@/stores/node-store";
import { PeerDialDialog } from "./peer-dial-dialog";
import { PeerDetailSheet } from "./peer-detail-sheet";
import { PeersTableCard } from "./peers-table-card";
import { PeersToolbar, type StatusFilter } from "./peers-toolbar";
import { peerStatusKind, type PeerStatusKind } from "./peer-status";
import { useTicker } from "@/views/network/use-ticker";

const PING_TIMEOUT_MS = 8000;

function matchesSearch(
  peer: { peerId: string; addrs: string[] },
  query: string,
  knownName: string | null,
): boolean {
  const q = query.trim().toLowerCase();
  if (q.length === 0) return true;
  return (
    peer.peerId.toLowerCase().includes(q) ||
    peer.addrs.some((addr) => addr.toLowerCase().includes(q)) ||
    (knownName !== null && knownName.toLowerCase().includes(q))
  );
}

export function PeersView() {
  const { i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const peers = useNodeStore(selectPeerList);
  // N-01：搜索纳入好友昵称/备注——首列显示人可读名，按屏上名字搜必须命中
  const friends = usePeerNameSource();
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

  // 跨卡 URL 契约：#/network/peers?dial=<目标> 挂载即开拨号弹窗并预填三段；
  // ?dial=1 保持兼容（仅打开）。关闭即清参，避免重挂载重复弹出。
  const dialParam = searchParams.get("dial");
  const dialOpen = dialParam !== null;
  const dialTarget = dialParam !== null && dialParam !== "1" ? dialParam : null;
  const setDialOpen = (open: boolean) => {
    if (open) setSearchParams(dialTarget ? { dial: dialTarget } : { dial: "1" });
    else setSearchParams({});
  };

  const filtered = peers.filter(
    (peer) =>
      matchesSearch(peer, query, peerKnownName(peer.peerId, friends)) &&
      (statusFilter === "all" || peerStatusKind(peer, now) === statusFilter),
  );

  // tabs 计数只按搜索词统计（不受当前状态过滤影响，否则未选 tab 恒为 0）。
  const tabCounts = useMemo(() => {
    const matched = peers.filter((peer) => matchesSearch(peer, query, peerKnownName(peer.peerId, friends)));
    const byKind = (kind: PeerStatusKind) =>
      matched.filter((peer) => peerStatusKind(peer, now) === kind).length;
    return {
      all: matched.length,
      connected: byKind("connected"),
      discovered: byKind("discovered"),
      offline: byKind("offline"),
    } as const;
  }, [peers, query, now]);
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
        counts={tabCounts}
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

      <PeerDialDialog open={dialOpen} onOpenChange={setDialOpen} initialTarget={dialTarget} />
      <PeerDetailSheet
        peer={detailPeer}
        onOpenChange={(open) => {
          if (!open) setDetailId(null);
        }}
      />
    </>
  );
}
