import { useCallback } from "react";
import { useTranslation } from "react-i18next";

import { useConfirm } from "@/components/feedback/confirm-provider";
import { toastSuccess } from "@/components/feedback/toast";
import type { NodeEventJson } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";
import { exportEventsJson } from "./events-export";

interface EventsCommandsInput {
  live: NodeEventJson[];
  paused: boolean;
  filtered: NodeEventJson[];
  onSnapshotClear: () => void;
}

// 事件流命令：清空（AlertDialog 确认，直写 store 公共 setState）与导出 JSON。
export function useEventsCommands(input: EventsCommandsInput) {
  const { t } = useTranslation();
  const confirm = useConfirm();

  const clearEvents = useCallback(async () => {
    const ok = await confirm({
      title: t("events.clearConfirm.title"),
      description: t("events.clearConfirm.description", {
        count: input.live.length,
      }),
      confirmText: t("common.actions.confirm"),
      cancelText: t("common.actions.cancel"),
      destructive: true,
    });
    if (!ok) return;
    useNodeStore.setState({ events: [] });
    if (input.paused) input.onSnapshotClear();
    toastSuccess(t("events.cleared"));
  }, [confirm, t, input]);

  const exportJson = useCallback(() => {
    exportEventsJson(input.filtered);
    toastSuccess(t("events.exported", { count: input.filtered.length }));
  }, [t, input]);

  return { clearEvents, exportJson };
}
