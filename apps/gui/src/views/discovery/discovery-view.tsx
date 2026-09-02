import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import { PageHeader } from "@/components/page/page-header";
import { ipc } from "@/lib/ipc";
import type { GuiConfig } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";
import { DiscoveredTableCard } from "./discovered-table-card";
import { MdnsCard } from "./mdns-card";
import { RendezvousCard } from "./rendezvous-card";
import { LoadFailedNotice } from "@/views/shared/load-state";
import { useGuiConfig } from "@/views/shared/use-gui-config";

// 发现页：mDNS 开关与结果表（store 事件派生）+ rendezvous 地址簿。
export function DiscoveryView() {
  const { t } = useTranslation();
  const { config, failed, reload } = useGuiConfig();
  const [localConfig, setLocalConfig] = useState<GuiConfig | null>(null);
  const effective = localConfig ?? config;
  const running = useNodeStore((s) => s.status?.running ?? false);

  const persistBootstrap = useCallback(
    async (bootstrap: string[]): Promise<boolean> => {
      if (!effective) return false;
      try {
        const saved = await ipc.configSave({ ...effective, bootstrap });
        setLocalConfig(saved);
        toastSuccess(t("discovery.rendezvous.saved"));
        return true;
      } catch (error) {
        console.error("[discovery] rendezvous 地址簿保存失败", error);
        toastError(
          t("discovery.rendezvous.saveFailed"),
          error instanceof Error ? error.message : String(error),
        );
        return false;
      }
    },
    [effective, t],
  );

  return (
    <>
      <PageHeader
        titleKey="discovery.title"
        descriptionKey="discovery.description"
      />
      {failed ? (
        <LoadFailedNotice onRetry={reload} messageKey="discovery.loadFailed" />
      ) : (
        <>
          <MdnsCard config={effective} onSaved={setLocalConfig} />
          <RendezvousCard
            bootstrap={effective?.bootstrap ?? []}
            running={running}
            onChange={persistBootstrap}
          />
          <DiscoveredTableCard />
        </>
      )}
    </>
  );
}
