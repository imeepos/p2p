import { useTranslation } from "react-i18next";

import { CopyButton } from "@/components/monitor/copy-button";
import { StatCard } from "@/components/page/stat-card";
import { Badge } from "@/components/ui/badge";
import type { Locale } from "@/i18n";
import { formatUptime } from "@/lib/format";
import type { NodeStatus } from "@/lib/ipc-types";
import { useTicker } from "./use-ticker";

const SHORT_PEER_ID_LEN = 12;

function PeerIdValue({ peerId }: { peerId: string }) {
  return (
    <span className="flex items-center gap-1">
      <span title={peerId}>{peerId.slice(0, SHORT_PEER_ID_LEN)}…</span>
      <CopyButton value={peerId} className="size-6" />
    </span>
  );
}

function ListenAddrList({ addrs }: { addrs: string[] }) {
  if (addrs.length === 0) return null;
  return (
    <ul className="flex flex-col gap-0.5">
      {addrs.map((addr) => (
        <li key={addr} className="break-all">
          {addr}
        </li>
      ))}
    </ul>
  );
}

// 状态卡 x4：运行状态、PeerId 复制、监听地址列表、运行时长（1s 跳动计时）。
export function DashboardStatusCards({ status }: { status: NodeStatus | null }) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const now = useTicker(1000);
  const loading = status === null;
  const running = status?.running ?? false;
  const peerId = status?.peerId ?? null;
  const uptimeSecs =
    running && status?.startedAtMs
      ? Math.max(0, Math.floor((now - status.startedAtMs) / 1000))
      : 0;

  return (
    <>
      <StatCard
        span={3}
        label={t("dashboard.cards.status")}
        loading={loading}
        value={
          <Badge variant={running ? "default" : "outline"}>
            {running ? t("common.state.running") : t("common.state.stopped")}
          </Badge>
        }
      />
      <StatCard
        span={3}
        label={t("dashboard.cards.peerId")}
        loading={loading}
        mono
        value={peerId ? <PeerIdValue peerId={peerId} /> : t("common.state.unknown")}
      />
      <StatCard
        span={3}
        label={t("dashboard.cards.listenAddrs")}
        loading={loading}
        mono
        value={
          status ? (
            <ListenAddrList addrs={status.listenAddrs} />
          ) : undefined
        }
      />
      <StatCard
        span={3}
        label={t("dashboard.cards.uptime")}
        loading={loading}
        value={formatUptime(running ? uptimeSecs : 0, locale)}
      />
    </>
  );
}
