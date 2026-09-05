import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { useAcpStore } from "@/acp/acp-store";
import { gradePermissionOption, type PermissionOptionGrade } from "@/acp/permission-grading";
import {
  permissionSecondsLeft,
  type PermissionRequestView,
} from "@/acp/interaction-model";
import { useConfirm } from "@/components/feedback/confirm-provider";
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

/** 分级强调色：allow_always 走 warning、reject 走 destructive，与 allow_once 的
 *  次级灰底拉开三档视觉差（P1 应答分级：不得全用同一主色） */
const TONE_CLASS: Record<NonNullable<PermissionOptionGrade["tone"]>, string> = {
  warning: "border-warning/50 text-warning hover:bg-warning/10",
  danger: "border-destructive/40 text-destructive hover:bg-destructive/10",
};

/** 存在 reject_* 选项时拒绝走 selected outcome；无 reject 才渲染通用拒绝（cancelled） */
function hasRejectOption(req: PermissionRequestView): boolean {
  return req.options.some((o) => typeof o.kind === "string" && o.kind.startsWith("reject"));
}

/** 单个应答选项：分级样式；allow_always 单击只弹确认框，确认后才应答（P1 两步门槛） */
function OptionButton(props: {
  req: PermissionRequestView;
  optionId: string;
  name: string;
  kind: string;
}) {
  const { t } = useTranslation();
  const respond = useAcpStore((s) => s.respondPermission);
  const confirm = useConfirm();
  const grade = gradePermissionOption({ kind: props.kind });

  const apply = () => respond(props.req.requestId, props.optionId);
  const onClick = () => {
    if (!grade.needsConfirm) {
      apply();
      return;
    }
    void confirm({
      title: t("acp.permission.confirmAlwaysTitle"),
      description: t("acp.permission.confirmAlwaysDescription", { name: props.name }),
      confirmText: t("acp.permission.confirmAlwaysConfirm"),
      cancelText: t("acp.cancel"),
      destructive: true,
    }).then((ok) => {
      if (ok) apply();
    });
  };

  return (
    <Button
      size="sm"
      variant={grade.variant}
      className={grade.tone ? TONE_CLASS[grade.tone] : undefined}
      onClick={onClick}
      data-perm-action={grade.action}
      data-testid={"acp-permission-option-" + props.req.requestId + "-" + props.optionId}
    >
      {props.name}
    </Button>
  );
}

function PendingActions({ req }: { req: PermissionRequestView }) {
  const { t } = useTranslation();
  const respond = useAcpStore((s) => s.respondPermission);
  return (
    <div className="flex flex-wrap gap-2">
      {req.options.map((opt) => (
        <OptionButton
          key={opt.optionId}
          req={req}
          optionId={opt.optionId}
          name={opt.name}
          kind={opt.kind}
        />
      ))}
      {hasRejectOption(req) ? null : (
        <Button
          size="sm"
          variant="outline"
          className={TONE_CLASS.danger}
          onClick={() => respond(req.requestId, null)}
          data-perm-action="reject"
          data-testid={"acp-permission-reject-" + req.requestId}
        >
          {t("acp.permission.reject")}
        </Button>
      )}
    </div>
  );
}

/** 单条权限请求：倒计时仅做展示（1s tick），归零应答由 store 侧 sweeper 负责 */
function PermissionRow({ req }: { req: PermissionRequestView }) {
  const { t } = useTranslation();
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
          <PendingActions req={req} />
        </>
      ) : null}
    </div>
  );
}

export function PermissionPanel() {
  const { t } = useTranslation();
  const cardRef = useRef<HTMLDivElement | null>(null);
  const seenPending = useRef<Set<number>>(new Set());
  const permissions = useAcpStore((s) =>
    s.activeSessionId ? s.interactions[s.activeSessionId]?.permissions : undefined,
  );
  const pendingKey = (permissions ?? [])
    .filter((p) => p.status === "pending")
    .map((p) => p.requestId)
    .join(",");

  // 新待决权限到达：面板滚入视口（P1）。用户回看历史时另有 toast 提醒兜底，
  // 不再只看到 agent 卡住等倒计时归零被静默拒绝
  useEffect(() => {
    if (pendingKey === "") return;
    const fresh = pendingKey
      .split(",")
      .map(Number)
      .filter((id) => !seenPending.current.has(id));
    if (fresh.length === 0) return;
    for (const id of fresh) seenPending.current.add(id);
    cardRef.current?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [pendingKey]);

  if (!permissions || permissions.length === 0) return null;
  return (
    <div ref={cardRef}>
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
    </div>
  );
}
