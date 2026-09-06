import { useTranslation } from "react-i18next";
import { useCallback, useState } from "react";

import { CopyButton } from "@/components/monitor/copy-button";
import { StatCard } from "@/components/page/stat-card";
import { Button } from "@/components/ui/button";
import type { Locale } from "@/i18n";
import { formatUptime } from "@/lib/format";
import type { NodeStatus } from "@/lib/ipc-types";
import { cn } from "@/lib/utils";
import { useNodeStore } from "@/stores/node-store";
import { StartNodeButton } from "@/views/network/start-node-button";
import { StopNodeDialog } from "@/views/network/stop-node-dialog";
import { useTicker } from "@/views/network/use-ticker";
import { StatusBadge } from "@/views/shared/status-badge";
import { PEER_ID_PREFIX_LEN } from "@/views/shared/peer-id-cell";

function PeerIdValue({ peerId }: { peerId: string }) {
  return (
    <span className="flex items-center gap-1">
      <span title={peerId}>{peerId.slice(0, PEER_ID_PREFIX_LEN)}…</span>
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

// 启停动作区（已拍板项 5）：启停常驻概览状态卡，高频运维就地可达；
// 停止走 StopNodeDialog 二次确认，失败路径 toast + console 显式可观测。
function StatusActions({ status }: { status: NodeStatus | null }) {
  const { t } = useTranslation();
  const startNode = useNodeStore((s) => s.startNode);
  const [stopOpen, setStopOpen] = useState(false);

  const ready = status !== null;
  const running = status?.running ?? false;

  const onStart = useCallback(async () => {
    const current = useNodeStore.getState().status;
    if (!current) throw new Error("node status not loaded");
    await startNode(current.config);
  }, [startNode]);

  return (
    <>
      <div className="flex flex-wrap items-center gap-2">
        <StartNodeButton disabled={!ready || running} action={onStart} />
        <Button
          size="sm"
          variant="outline"
          disabled={!ready || !running}
          onClick={() => setStopOpen(true)}
        >
          {t("common.actions.stop")}
        </Button>
      </div>
      <StopNodeDialog open={stopOpen} onOpenChange={setStopOpen} />
    </>
  );
}

// 节点状态卡组（4.2 第 1 块）：运行状态、PeerId 复制、监听地址列表、
// 运行时长（1s 跳动计时），启停按钮挂在运行状态卡底部动作区。
export function OverviewStatusCards({ status }: { status: NodeStatus | null }) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const now = useTicker(1000);
  const loading = status === null;
  const running = status?.running ?? false;
  const peerId = status?.peerId ?? null;
  // F19：停止后后端 status.peerId 置 null，但身份种子仍在——记住最后一个
  // 运行期身份，停止态置灰保留展示并注明语义，不再落「未知」。
  // render 期条件回填是 React 官方「props 变化调整 state」模式：
  // 仅在身份实际变化时 setState，停止(null)不触发，保留即无闪烁。
  const [lastIdentity, setLastIdentity] = useState<string | null>(peerId);
  if (peerId !== null && peerId !== lastIdentity) {
    setLastIdentity(peerId);
  }
  const identity = peerId ?? lastIdentity;
  const identityRetained = peerId === null && identity !== null;
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
        footer={<StatusActions status={status} />}
        value={
          <StatusBadge tone={running ? "success" : "neutral"} dot>
            {running ? t("common.state.running") : t("common.state.stopped")}
          </StatusBadge>
        }
      />
      <StatCard
        span={3}
        label={t("dashboard.cards.peerId")}
        loading={loading}
        mono
        value={
          identity ? (
            <span
              className={cn(
                "flex flex-col gap-0.5",
                identityRetained && "opacity-60",
              )}
              data-testid="dashboard-identity"
            >
              <PeerIdValue peerId={identity} />
              {identityRetained ? (
                <span className="text-muted-foreground text-xs font-normal">
                  {t("peerName.identityKept")}
                </span>
              ) : null}
            </span>
          ) : (
            t("common.state.unknown")
          )
        }
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
