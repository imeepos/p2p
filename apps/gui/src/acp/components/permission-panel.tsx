import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { useAcpStore } from "@/acp/acp-store";
import {
  permissionSecondsLeft,
  type PermissionRequestView,
} from "@/acp/interaction-model";
import { StatusBadge, type StatusTone } from "@/views/shared/status-badge";

const STATUS_TONE: Record<PermissionRequestView["status"], StatusTone> = {
  pending: "warning",
  approved: "success",
  rejected: "danger",
};

const STATUS_KEY: Record<Exclude<PermissionRequestView["status"], "pending">, "acp.permission.approved" | "acp.permission.rejected"> = {
  approved: "acp.permission.approved",
  rejected: "acp.permission.rejected",
};

/** 存在 reject_* 选项时拒绝走 selected outcome；无 reject 才渲染通用拒绝（cancelled） */
function hasRejectOption(req: PermissionRequestView): boolean {
  return req.options.some((o) => typeof o.kind === "string" && o.kind.startsWith("reject"));
}

/** 单条权限请求：倒计时仅做展示（1s tick），归零应答由 store 侧 sweeper 负责 */
function PermissionRow({ req }: { req: PermissionRequestView }) {
  const { t } = useTranslation();
  const respond = useAcpStore((s) => s.respondPermission);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (req.status !== "pending") return;
    const timer = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(timer);
  }, [req.status]);

  return (
    <div className="flex flex-col gap-1 rounded-md border px-2 py-1.5"
      data-testid={"acp-permission-row-" + req.requestId}>
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-sm font-medium">{req.title}</span>
        {req.toolKind ? (
          <span className="bg-muted rounded px-1.5 py-0.5 text-xs">{req.toolKind}</span>
        ) : null}
        <span data-testid={"acp-permission-status-" + req.requestId}>
          <StatusBadge tone={STATUS_TONE[req.status]}>
            {req.status === "pending" ? t("acp.permission.pending") : t(STATUS_KEY[req.status])}
          </StatusBadge>
        </span>
      </div>
      {req.status === "pending" ? (
        <>
          <p className="text-muted-foreground text-xs" data-testid={"acp-permission-countdown-" + req.requestId}>
            {t("acp.permission.countdown", { secs: permissionSecondsLeft(req, now) })}
          </p>
          <div className="flex flex-wrap gap-2">
            {req.options.map((opt) => (
              <Button
                key={opt.optionId}
                size="sm"
                variant={opt.kind.startsWith("reject") ? "outline" : "default"}
                onClick={() => respond(req.requestId, opt.optionId)}
                data-testid={"acp-permission-option-" + req.requestId + "-" + opt.optionId}
              >
                {opt.name}
              </Button>
            ))}
            {hasRejectOption(req) ? null : (
              <Button size="sm" variant="outline" onClick={() => respond(req.requestId, null)}
                data-testid={"acp-permission-reject-" + req.requestId}>
                {t("acp.permission.reject")}
              </Button>
            )}
          </div>
        </>
      ) : null}
    </div>
  );
}

export function PermissionPanel() {
  const { t } = useTranslation();
  const permissions = useAcpStore((s) =>
    s.activeSessionId ? s.interactions[s.activeSessionId]?.permissions : undefined,
  );
  if (!permissions || permissions.length === 0) return null;
  return (
    <Card data-testid="acp-permission-panel">
      <CardHeader className="pb-2">
        <CardTitle className="text-base">{t("acp.permission.card")}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {permissions.map((req) => (
          <PermissionRow key={req.requestId} req={req} />
        ))}
      </CardContent>
    </Card>
  );
}