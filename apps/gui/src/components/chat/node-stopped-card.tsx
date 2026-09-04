import { useState } from "react";

import { AsyncButton } from "@/components/feedback/async-button";
import { useNodeStore } from "@/stores/node-store";
import { ipc } from "@/lib/ipc";
import { errorText } from "@/views/shared/form-flow";
import { useTranslation } from "react-i18next";

// 节点未运行引导卡（IM-T51）：节点停止时全部聊天命令 Err，输入区以显式
// 引导替代静默不可用；启动成功经 node-store status 自动恢复（卡片消失）。
export function NodeStoppedCard() {
  const { t } = useTranslation();
  const startNode = useNodeStore((s) => s.startNode);
  const [error, setError] = useState<string | null>(null);

  const start = async () => {
    setError(null);
    await startNode(await ipc.configGet());
  };

  return (
    <div data-testid="chat-node-stopped" className="border-t p-3">
      <p className="text-sm text-muted-foreground">{t("chat.nodeStopped.hint")}</p>
      <div className="mt-2 flex flex-wrap items-center gap-2">
        <AsyncButton
          type="button"
          size="sm"
          action={start}
          loadingLabel={t("chat.nodeStopped.starting")}
          onError={(cause) => {
            console.error("[chat] 节点启动失败", cause);
            setError(errorText(cause));
          }}
        >
          {t("chat.nodeStopped.start")}
        </AsyncButton>
        {error ? (
          <span className="text-xs text-destructive">
            {t("chat.nodeStopped.startFailed")}: {error}
          </span>
        ) : null}
      </div>
    </div>
  );
}
