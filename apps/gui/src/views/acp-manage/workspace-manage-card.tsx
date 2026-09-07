// 工作区管理卡：admin GET /workspaces 清单 + 新增 + 删除（实时生效，无需重启 agent）。
// 删除是破坏性操作，先过确认弹框；错误经词法码映射人话文案。
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { FolderGit2, RefreshCw } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useConfirm } from "@/components/feedback/confirm-provider";
import { toastError } from "@/components/feedback/toast";
import { listWorkspaces, removeWorkspace, type AcpWorkspace } from "@/acp/share-admin-client";
import { EmptyState } from "@/views/shared/empty-state";
import { useAdminEndpoint } from "./use-admin-endpoint";
import { WorkspaceAddForm } from "./workspace-add-form";
import { workspaceErrorKey } from "./workspace-errors";

export function WorkspaceManageCard() {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const { endpointUrl, endpointToken, done } = useAdminEndpoint(t("acpManage.localCandidateLabel"));
  const [rows, setRows] = useState<AcpWorkspace[] | null>(null);
  const [loadError, setLoadError] = useState(false);
  const [busy, setBusy] = useState(false);
  const [tick, setTick] = useState(0);
  const bump = useCallback(() => setTick((n) => n + 1), []);

  useEffect(() => {
    if (endpointUrl === null) return;
    let dead = false;
    listWorkspaces(endpointUrl, endpointToken)
      .then((list) => {
        if (dead) return;
        setRows(list);
        setLoadError(false);
      })
      .catch(() => {
        if (dead) return;
        setRows(null);
        setLoadError(true);
      });
    return () => {
      dead = true;
    };
  }, [endpointUrl, endpointToken, tick]);

  const remove = (row: AcpWorkspace) => {
    void confirm({
      title: t("acpManage.workspaces.removeConfirmTitle"),
      description: t("acpManage.workspaces.removeConfirmText", { id: row.id }),
      confirmText: t("acpManage.workspaces.remove"),
      cancelText: t("acp.cancel"),
      destructive: true,
    }).then(async (ok) => {
      if (!ok || endpointUrl === null) return;
      try {
        await removeWorkspace(endpointUrl, endpointToken, row.id);
        bump();
      } catch (err) {
        toastError(t(workspaceErrorKey(err)), { context: "acp-ws-remove" });
      }
    });
  };

  return (
    <Card data-testid="acp-manage-workspaces-card">
      <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-base">{t("acpManage.workspaces.title")}</CardTitle>
        <Button size="icon" variant="ghost" onClick={bump} aria-label={t("acp.local.refresh")} data-testid="acp-ws-refresh">
          <RefreshCw aria-hidden className="size-4" />
        </Button>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {endpointUrl === null && done ? (
          <p className="text-muted-foreground text-sm" data-testid="acp-ws-need-admin">
            {t("acpManage.status.noDescriptor")}
          </p>
        ) : loadError ? (
          <p className="text-destructive text-sm" data-testid="acp-ws-load-error">
            {t("acpManage.workspaces.loadFailed")}
          </p>
        ) : rows === null ? null : rows.length === 0 ? (
          <EmptyState icon={FolderGit2} title={t("acpManage.workspaces.empty")} description={t("acpManage.workspaces.emptyHint")} />
        ) : (
          rows.map((row) => (
            <div key={row.id} className="flex items-center justify-between gap-2 rounded-md border px-2 py-1.5" data-testid={"acp-ws-row-" + row.id}>
              <div className="min-w-0">
                <p className="truncate text-sm font-medium">{row.name}</p>
                <p className="text-muted-foreground truncate font-mono text-xs" title={row.dir}>{row.dir}</p>
              </div>
              <Button size="sm" variant="outline" onClick={() => remove(row)} data-testid={"acp-ws-remove-" + row.id}>
                {t("acpManage.workspaces.remove")}
              </Button>
            </div>
          ))
        )}
        {endpointUrl !== null ? (
          <WorkspaceAddForm
            adminUrl={endpointUrl}
            adminToken={endpointToken}
            busy={busy}
            setBusy={setBusy}
            onAdded={bump}
            onError={() => {}}
          />
        ) : null}
      </CardContent>
    </Card>
  );
}