import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { useAcpStore } from "@/acp/acp-store";
import { useConfirm } from "@/components/feedback/confirm-provider";
import { toastError } from "@/components/feedback/toast";
import { adminEndpointCandidates } from "@/acp/admin-endpoints";
import { listShares, revokeShare } from "@/acp/share-admin-client";
import { shareStatus, type ShareEntry, type ShareStatus } from "@/acp/share-model";
import type { I18nKey } from "@/i18n/types";
import type { Locale } from "@/i18n";
import { formatDateTime } from "@/lib/format";
import { RefreshCw } from "lucide-react";
import { StatusBadge, type StatusTone } from "@/views/shared/status-badge";
import { ShareCreateDialog } from "./share-create-dialog";

const STATUS_TONE: Record<ShareStatus, StatusTone> = {
  active: "success",
  bound: "warning",
  exhausted: "neutral",
  expired: "neutral",
  revoked: "danger",
};

const STATUS_KEY: Record<ShareStatus, I18nKey> = {
  active: "acp.share.manage.status.active",
  bound: "acp.share.manage.status.bound",
  exhausted: "acp.share.manage.status.exhausted",
  expired: "acp.share.manage.status.expired",
  revoked: "acp.share.manage.status.revoked",
};

function ShareRow({ entry }: { entry: ShareEntry }) {
  const { t, i18n } = useTranslation();
  const confirm = useConfirm();
  const status = shareStatus(entry, Math.floor(Date.now() / 1000));
  const expiresText = entry.expires_at_unix
    ? t("acp.share.manage.expiresAt", {
        time: formatDateTime(entry.expires_at_unix * 1000, i18n.language as Locale),
      })
    : "-";
  const revoke = () => {
    // 破坏性操作确认纪律（P2）：撤销先过确认弹框（级联语义见文案）
    void confirm({
      title: t("acp.share.manage.revokeConfirmTitle"),
      description: t("acp.share.manage.revokeConfirmDescription"),
      confirmText: t("acp.share.manage.revokeConfirmAction"),
      cancelText: t("acp.cancel"),
      destructive: true,
    }).then(async (ok) => {
      if (!ok) return;
      const { draft } = useAcpStore.getState();
      try {
        await revokeShare(draft.adminUrl ?? "", draft.adminToken ?? "", entry.share_id);
      } catch (error) {
        toastError(t("acp.share.manage.revokeFailed"), {
          description: error instanceof Error ? error.message : String(error),
          context: "acp-share-revoke",
        });
      }
    });
  };
  return (
    <div
      className="flex items-center justify-between gap-2 rounded-md border px-2 py-1.5"
      data-testid={"acp-share-row-" + entry.share_id}
    >
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium">{entry.note || t("acp.share.manage.noNote")}</p>
        <p className="text-muted-foreground truncate text-xs">
          {expiresText}
          {entry.bound_peer ? " · " + t("acp.share.manage.boundTo", { peer: entry.bound_peer.slice(0, 8) }) : ""}
        </p>
      </div>
      <div className="flex shrink-0 items-center gap-2">
        <StatusBadge tone={entry.scope === "workspace" ? "warning" : "neutral"}>
          {t(entry.scope === "workspace" ? "acp.share.scopeWorkspace" : "acp.share.scopeSandbox")}
        </StatusBadge>
        <span data-testid={"acp-share-status-" + entry.share_id}>
          <StatusBadge tone={STATUS_TONE[status]}>{t(STATUS_KEY[status])}</StatusBadge>
        </span>
        <span className="text-muted-foreground text-xs">
          {t("acp.share.manage.activations", { used: entry.activations, max: entry.max_activations })}
        </span>
        {!entry.revoked ? (
          <Button
            size="sm"
            variant="outline"
            onClick={revoke}
            data-testid={"acp-share-revoke-" + entry.share_id}
          >
            {t("acp.share.manage.revoke")}
          </Button>
        ) : null}
      </div>
    </div>
  );
}

/** owner 分享管理卡（§8）：admin GET /shares 台账 + 状态徽章 + 撤销 +
 *  与聊天页同源的创建入口。admin 端点来自既有 endpoint 登记（草稿优先）。 */
export function ShareManageCard() {
  const { t } = useTranslation();
  const saved = useAcpStore((s) => s.saved);
  const draft = useAcpStore((s) => s.draft);
  const endpoint = adminEndpointCandidates(saved, draft)[0] ?? null;
  // effect 依赖只取原始值：endpoint 对象每渲染都是新引用，直接依赖会自旋
  const endpointUrl = endpoint?.url ?? null;
  const endpointToken = endpoint?.token ?? "";
  const [rows, setRows] = useState<ShareEntry[] | null>(null);
  const [loadError, setLoadError] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);

  const reload = useCallback(async () => {
    if (!endpointUrl) return;
    try {
      const list = await listShares(endpointUrl, endpointToken);
      setRows(list);
      setLoadError(false);
    } catch (error) {
      console.warn("[acp] share list load failed", error);
      setRows(null);
      setLoadError(true);
    }
  }, [endpointUrl, endpointToken]);

  useEffect(() => {
    void reload();
  }, [reload]);

  return (
    <Card data-testid="acp-share-manage-card">
      <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-base">{t("acp.share.manage.card")}</CardTitle>
        <div className="flex items-center gap-1">
          <Button
            size="icon"
            variant="ghost"
            onClick={() => void reload()}
            aria-label={t("acp.share.manage.refresh")}
            data-testid="acp-share-manage-refresh"
          >
            <RefreshCw aria-hidden className="size-4" />
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={() => setCreateOpen(true)}
            data-testid="acp-share-manage-create"
          >
            {t("acp.share.manage.create")}
          </Button>
        </div>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {!endpoint ? (
          <p className="text-muted-foreground text-sm" data-testid="acp-share-manage-need-admin">
            {t("acp.share.manage.needAdmin")}
          </p>
        ) : loadError ? (
          <div className="flex items-center justify-between gap-2">
            <p className="text-destructive text-sm" data-testid="acp-share-manage-error">
              {t("acp.share.manage.loadFailed")}
            </p>
            <Button size="sm" variant="outline" onClick={() => void reload()} data-testid="acp-share-manage-reload">
              {t("acp.share.manage.reload")}
            </Button>
          </div>
        ) : rows === null ? null : rows.length === 0 ? (
          <p className="text-muted-foreground text-sm" data-testid="acp-share-manage-empty">
            {t("acp.share.manage.empty")}
          </p>
        ) : (
          rows.map((entry) => <ShareRow key={entry.share_id} entry={entry} />)
        )}
      </CardContent>
      <ShareCreateDialog open={createOpen} onOpenChange={(open) => { setCreateOpen(open); if (!open) void reload(); }} />
    </Card>
  );
}
