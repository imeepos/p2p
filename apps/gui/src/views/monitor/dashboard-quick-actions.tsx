import { useTranslation } from "react-i18next";
import { useCallback, useState } from "react";
import { Link } from "react-router-dom";

import { Button } from "@/components/ui/button";
import { useNodeStore } from "@/stores/node-store";
import { StopNodeDialog } from "./stop-node-dialog";
import { StartNodeButton } from "./start-node-button";

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
      <StartNodeButton disabled={!ready || running} action={onStart} />
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
