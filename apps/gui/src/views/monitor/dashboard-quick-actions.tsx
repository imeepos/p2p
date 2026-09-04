import { useTranslation } from "react-i18next";
import { useCallback, useState } from "react";
import { Link } from "react-router-dom";

import { AsyncButton } from "@/components/feedback/async-button";
import { toastError, toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import { useNodeStore } from "@/stores/node-store";
import { errorText } from "@/views/shared/form-flow";
import { StopNodeDialog } from "./stop-node-dialog";

// 顶部快速操作：启动直接执行；停止走二次确认弹框；拨号入口跳节点页。
export function DashboardQuickActions() {
  const { t } = useTranslation();
  const status = useNodeStore((s) => s.status);
  const startNode = useNodeStore((s) => s.startNode);
  const [stopOpen, setStopOpen] = useState(false);

  const running = status?.running ?? false;
  const ready = status !== null;

  const onStart = useCallback(async () => {
    if (!status) throw new Error("node status not loaded");
    await startNode(status.config);
  }, [status, startNode]);

  return (
    <div className="col-span-12 flex flex-wrap items-center gap-2">
      <AsyncButton
        size="sm"
        disabled={!ready || running}
        action={onStart}
        loadingLabel={t("common.state.starting")}
        onSuccess={() => toastSuccess(t("common.actions.startSucceeded"))}
        onError={(error) => {
          console.error("[dashboard] 启动节点失败", error);
          toastError(t("common.actions.startFailed"), {
            description: errorText(error),
            context: "node.start",
          });
        }}
      >
        {t("common.actions.start")}
      </AsyncButton>
      <Button
        size="sm"
        variant="outline"
        disabled={!ready || !running}
        onClick={() => setStopOpen(true)}
      >
        {t("common.actions.stop")}
      </Button>
      <Button size="sm" variant="outline" asChild>
        <Link to="/peers?dial=1">{t("peers.dial.title")}</Link>
      </Button>

      <StopNodeDialog open={stopOpen} onOpenChange={setStopOpen} />
    </div>
  );
}
