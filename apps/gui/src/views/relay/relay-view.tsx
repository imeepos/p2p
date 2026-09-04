import { useCallback, useState } from "react";

import { PageHeader } from "@/components/page/page-header";
import { Skeleton } from "@/components/ui/skeleton";
import { markLocalWrite } from "@/lib/data-watch";
import { ipc } from "@/lib/ipc";
import type { GuiConfig } from "@/lib/ipc-types";
import { ChainNoteCard } from "./chain-note-card";
import { HopStatsCard } from "./hop-stats-card";
import { RelayConfigCard } from "./relay-config-card";
import { RelayWatermarkCard } from "./relay-watermark-card";
import { LoadFailedNotice } from "@/views/shared/load-state";
import { useGuiConfig } from "@/views/shared/use-gui-config";

// 中继页：relay 地址配置、会话水位、逐跳比例与降级链说明。
export function RelayView() {
  const { config, failed, reload } = useGuiConfig();
  const [localConfig, setLocalConfig] = useState<GuiConfig | null>(null);
  const effective = localConfig ?? config;

  // 保存失败 toast 在 RelayConfigCard 的 onError；此处保留 console 信号并上抛。
  const persistRelay = useCallback(
    async (relayAddrs: string[]) => {
      if (!effective) return;
      try {
        const saved = await ipc.configSave({ ...effective, relayAddrs });
        markLocalWrite("config");
        setLocalConfig(saved);
      } catch (error) {
        console.error("[relay] relayAddrs 保存失败", error);
        throw error;
      }
    },
    [effective],
  );

  return (
    <>
      <PageHeader titleKey="relay.title" descriptionKey="relay.description" />
      {failed ? (
        <LoadFailedNotice onRetry={reload} messageKey="relay.loadFailed" />
      ) : effective === null ? (
        <>
          <Skeleton className="col-span-12 h-56 lg:col-span-6" />
          <Skeleton className="col-span-12 h-56 lg:col-span-6" />
          <Skeleton className="col-span-12 h-40" />
        </>
      ) : (
        <>
          <RelayConfigCard
            relayAddrs={effective.relayAddrs}
            onSave={persistRelay}
          />
          <RelayWatermarkCard />
          <HopStatsCard />
          <ChainNoteCard />
        </>
      )}
    </>
  );
}