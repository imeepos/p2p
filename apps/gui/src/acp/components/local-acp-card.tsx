import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { FolderGit2, RefreshCw, Settings2, Share2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { AsyncButton } from "@/components/feedback/async-button";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { errorText } from "@/views/shared/form-flow";
import { useAcpStore } from "@/acp/acp-store";
import { listWorkspaces, type AcpWorkspace } from "@/acp/share-admin-client";
import { useLocalAdminCandidate } from "@/acp/use-local-admin";
import { EmptyState } from "@/views/shared/empty-state";
import { ShareCreateDialog } from "./share-create-dialog";

/** 本地 ACP 卡（信息架构：本地/远程分区的本地区主体）：
 *  自动发现本机 agent admin 端点，列出可分享工作区，每行可发起定向分享。 */
export function LocalAcpCard() {
  const { t } = useTranslation();
  const saved = useAcpStore((s) => s.saved);
  const draft = useAcpStore((s) => s.draft);
  const registered = adminFirst(saved, draft);
  const { candidate: localCandidate, done: localDone } = useLocalAdminCandidate(
    registered === null,
    t("acp.share.localCandidateLabel"),
  );
  const endpoint = registered ?? localCandidate;
  const endpointUrl = endpoint?.url ?? null;
  const endpointToken = endpoint?.token ?? "";

  const [rows, setRows] = useState<AcpWorkspace[] | null>(null);
  const [loadError, setLoadError] = useState(false);
  const [tick, setTick] = useState(0);
  const [shareOpen, setShareOpen] = useState(false);
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState<string | undefined>();
  const bump = useCallback(() => setTick((n) => n + 1), []);

  // 工作区清单：effect 兜 endpoint 变化/弹层关闭后的重取；刷新按钮走 load
  // （AsyncButton 需要 reject 语义）。失败双留痕：按钮路径 toast，两路共用
  // 行内错误提示；404（旧 agent 无端点）契约性回落空态不算失败。
  const load = useCallback(async () => {
    if (!endpointUrl) return;
    const list = await listWorkspaces(endpointUrl, endpointToken);
    setRows(list);
    setLoadError(false);
  }, [endpointUrl, endpointToken]);

  useEffect(() => {
    if (!endpointUrl) return;
    let dead = false;
    listWorkspaces(endpointUrl, endpointToken)
      .then((list) => {
        if (dead) return;
        setRows(list);
        setLoadError(false);
      })
      .catch((error) => {
        console.warn("[acp] workspace list load failed", error);
        if (dead) return;
        setLoadError(true);
      });
    return () => {
      dead = true;
    };
  }, [endpointUrl, endpointToken, tick]);

  return (
    <Card data-testid="acp-local-card">
      <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-base">{t("acp.local.card")}</CardTitle>
        <div className="flex items-center gap-1">
          <Button
            size="icon"
            variant="ghost"
            aria-label={t("acp.local.manage")}
            title={t("acp.local.manage")}
            data-testid="acp-local-manage"
            onClick={() => {
              // HashRouter 场景（App 用 createHashRouter）；不用 Link 以保持
              // 本卡在无 Router 的测试环境可渲染。
              window.location.hash = "#/acp-manage";
            }}
          >
            <Settings2 aria-hidden className="size-4" />
          </Button>
          <AsyncButton
            size="icon"
            variant="ghost"
            iconOnly
            action={load}
            onSuccess={() => toastSuccess(t("chat.feedback.actions.refreshed"))}
            onError={(error) => {
              setLoadError(true);
              toastError(t("chat.feedback.actions.refreshFailed"), {
                description: errorText(error),
                context: "acp-local-workspace-list",
              });
            }}
            aria-label={t("acp.local.refresh")}
            data-testid="acp-local-refresh"
          >
            <RefreshCw aria-hidden className="size-4" />
          </AsyncButton>
          <Button
            size="sm"
            variant="outline"
            disabled={!endpointUrl}
            onClick={() => { setSelectedWorkspaceId(undefined); setShareOpen(true); }}
            data-testid="acp-local-share"
          >
            <Share2 aria-hidden className="size-4" />
            {t("acp.local.shareAction")}
          </Button>
        </div>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {loadError ? (
          <p
            className="text-destructive text-xs"
            role="alert"
            data-testid="acp-local-workspace-error"
          >
            {t("chat.feedback.actions.refreshFailed")}
          </p>
        ) : null}
        {!endpoint && localDone ? (
          <p className="text-muted-foreground text-sm" data-testid="acp-local-need-agent">
            {t("acp.local.empty")}
          </p>
        ) : rows === null ? null : rows.length === 0 ? (
          <div data-testid="acp-local-workspace-empty">
            <EmptyState
              icon={FolderGit2}
              title={t("acp.local.workspaceEmpty")}
              description={t("acp.local.workspaceEmptyHint")}
            />
          </div>
        ) : (
          rows.map((ws) => (
            <div
              key={ws.id}
              className="flex items-center justify-between gap-2 rounded-md border px-2 py-1.5"
              data-testid={"acp-local-workspace-" + ws.id}
            >
              <div className="min-w-0">
                <p className="truncate text-sm font-medium">{ws.name}</p>
                <p className="text-muted-foreground truncate text-xs">{ws.dir}</p>
              </div>
              <Button
                size="sm"
                variant="outline"
                onClick={() => { setSelectedWorkspaceId(ws.id); setShareOpen(true); }}
                data-testid={"acp-local-share-" + ws.id}
              >
                {t("acp.local.shareAction")}
              </Button>
            </div>
          ))
        )}
      </CardContent>
      <ShareCreateDialog
        open={shareOpen}
        initialWorkspaceId={selectedWorkspaceId}
        onOpenChange={(open) => { setShareOpen(open); if (!open) bump(); }}
      />
    </Card>
  );
}

function adminFirst(
  saved: ReturnType<typeof useAcpStore.getState>["saved"],
  draft: ReturnType<typeof useAcpStore.getState>["draft"],
) {
  for (const ep of [draft, ...saved]) {
    const url = ep.adminUrl?.trim();
    if (url) return { url, token: ep.adminToken?.trim() ?? "" };
  }
  return null;
}