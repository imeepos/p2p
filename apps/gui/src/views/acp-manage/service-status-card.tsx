// 本机 agent 服务状态卡：描述文件事实 + admin 通道在线探测（GET /workspaces 探针）。
// 探针兼作二进制新鲜度检查：老二进制无 /workspaces 端点会 404，UI 显式提示刷新。
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw, Server } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { StatusBadge } from "@/views/shared/status-badge";
import { ipc } from "@/lib/ipc";
import type { AcpLocalDescriptor } from "@/lib/ipc-types";
import { listWorkspaces } from "@/acp/share-admin-client";
import { useAdminEndpoint } from "./use-admin-endpoint";

type ProbeState = "probing" | "online" | "stale" | "offline" | "none";

const PROBE_KEYS: Record<ProbeState, string> = {
  probing: "acpManage.status.probing",
  online: "acpManage.status.online",
  stale: "acpManage.status.stale",
  offline: "acpManage.status.offline",
  none: "acpManage.status.offline",
};

const PROBE_TONES: Record<ProbeState, "success" | "neutral" | "danger"> = {
  probing: "neutral",
  online: "success",
  stale: "danger",
  offline: "danger",
  none: "danger",
};

export function ServiceStatusCard() {
  const { t } = useTranslation();
  const { endpointUrl, endpointToken, done } = useAdminEndpoint(t("acpManage.localCandidateLabel"));
  const [descriptor, setDescriptor] = useState<AcpLocalDescriptor | null>(null);
  const [probe, setProbe] = useState<ProbeState>("probing");
  const [tick, setTick] = useState(0);
  const bump = useCallback(() => setTick((n) => n + 1), []);

  useEffect(() => {
    let dead = false;
    ipc
      .acpLocalDescriptor()
      .then((value) => {
        if (!dead) setDescriptor(value);
      })
      .catch(() => {
        if (!dead) setDescriptor(null);
      });
    return () => {
      dead = true;
    };
  }, [tick]);

  useEffect(() => {
    if (endpointUrl === null) {
      return;
    }
    let dead = false;
    const run = async () => {
      setProbe("probing");
      try {
        await listWorkspaces(endpointUrl, endpointToken);
        if (!dead) setProbe("online");
      } catch (error: unknown) {
        if (dead) return;
        setProbe(
          error instanceof Error && error.message.includes("404") ? "stale" : "offline",
        );
      }
    };
    void run();
    return () => {
      dead = true;
    };
  }, [endpointUrl, endpointToken, done, tick]);

  // 无端点且发现已收尾 = none；不在 effect 里 setState（级联渲染纪律）。
  const effective: ProbeState =
    endpointUrl === null && done ? "none" : probe;

  return (
    <Card data-testid="acp-manage-status-card">
      <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-base">{t("acpManage.status.title")}</CardTitle>
        <div className="flex items-center gap-2">
          <span data-testid="acp-manage-status-probe">
            <StatusBadge tone={PROBE_TONES[effective]}>{t(PROBE_KEYS[effective] as never)}</StatusBadge>
          </span>
          <Button
            size="icon"
            variant="ghost"
            onClick={bump}
            aria-label={t("acpManage.status.refresh")}
            data-testid="acp-manage-status-refresh"
          >
            <RefreshCw aria-hidden className="size-4" />
          </Button>
        </div>
      </CardHeader>
      <CardContent className="flex flex-col gap-1.5">
        {descriptor ? (
          <>
            <StatusRow label={t("acpManage.status.nameLabel")} value={descriptor.agentName} />
            <StatusRow label={t("acpManage.status.peerLabel")} value={descriptor.peer} mono />
            <StatusRow label={t("acpManage.status.adminLabel")} value={descriptor.adminUrl} mono />
          </>
        ) : (
          <div className="flex items-start gap-2 rounded-md border px-2 py-1.5" data-testid="acp-manage-status-none">
            <Server aria-hidden className="text-muted-foreground mt-0.5 size-4 shrink-0" />
            <p className="text-muted-foreground text-sm">{t("acpManage.status.noDescriptor")}</p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function StatusRow(props: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="flex min-w-0 items-baseline gap-2 text-sm">
      <span className="text-muted-foreground w-20 shrink-0">{props.label}</span>
      <span className={props.mono ? "min-w-0 truncate font-mono text-xs" : "min-w-0 truncate"} title={props.value}>
        {props.value}
      </span>
    </div>
  );
}