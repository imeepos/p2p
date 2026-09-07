import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { FolderGit2, RefreshCw, Share2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
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
  const [tick, setTick] = useState(0);
  const [shareOpen, setShareOpen] = useState(false);
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState<string | undefined>();
  const bump = useCallback(() => setTick((n) => n + 1), []);

  useEffect(() => {
    if (!endpointUrl) return;
    let dead = false;
    listWorkspaces(endpointUrl, endpointToken)
      .then((list) => {
        if (!dead) setRows(list);
      })
      .catch(() => {
        if (!dead) setRows([]);
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
            onClick={bump}
            aria-label={t("acp.local.refresh")}
            data-testid="acp-local-refresh"
          >
            <RefreshCw aria-hidden className="size-4" />
          </Button>
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