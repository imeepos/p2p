import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import { PageHeader } from "@/components/page/page-header";
import { Skeleton } from "@/components/ui/skeleton";
import { markLocalWrite } from "@/lib/data-watch";
import { ipc } from "@/lib/ipc";
import type { GuiConfig } from "@/lib/ipc-types";
import { useNodeStore } from "@/stores/node-store";
import { errorText } from "@/views/shared/form-flow";
import { DiscoveredTableCard } from "./discovered-table-card";
import { MdnsCard } from "./mdns-card";
import { RendezvousCard } from "./rendezvous-card";
import { LoadFailedNotice } from "@/views/shared/load-state";
import { useUnsavedGuard } from "@/views/shared/use-unsaved-guard";
import { useGuiConfig } from "@/views/shared/use-gui-config";

// 发现页：mDNS 开关与设置页统一「置脏 + 保存条」模型（切换只置脏，保存落盘，
// 重启节点生效）；结果表（store 事件派生）+ rendezvous 地址簿；空态入口跨卡联动。
export function DiscoveryView() {
  const { t } = useTranslation();
  const running = useNodeStore((s) => s.status?.running ?? false);
  const { config, failed, reload } = useGuiConfig();
  const [localConfig, setLocalConfig] = useState<GuiConfig | null>(null);
  const [mdnsDraft, setMdnsDraft] = useState<boolean | null>(null);
  const [addOpen, setAddOpen] = useState(false);
  const effective = localConfig ?? config;

  useUnsavedGuard("discovery-mdns", {
    hasUnsaved: () => mdnsDraft !== null,
    discard: () => setMdnsDraft(null),
  });

  const persistBootstrap = useCallback(
    async (bootstrap: string[]): Promise<boolean> => {
      if (!effective) return false;
      try {
        const saved = await ipc.configSave({ ...effective, bootstrap });
        markLocalWrite("config");
        setLocalConfig(saved);
        toastSuccess(t("discovery.rendezvous.saved"));
        return true;
      } catch (error) {
        console.error("[discovery] rendezvous 地址簿保存失败", error);
        toastError(t("discovery.rendezvous.saveFailed"), {
          description: errorText(error),
          context: "discovery.bootstrap_save",
        });
        return false;
      }
    },
    [effective, t],
  );

  const saveMdns = useCallback(async () => {
    if (!effective || mdnsDraft === null) return;
    try {
      const saved = await ipc.configSave({
        ...effective,
        enableMdns: mdnsDraft,
      });
      markLocalWrite("config");
      setLocalConfig(saved);
      setMdnsDraft(null);
      toastSuccess(t("discovery.mdns.saved"));
    } catch (error) {
      console.error("[discovery] mDNS 开关保存失败", error);
      toastError(t("discovery.mdns.saveFailed"), {
        description: errorText(error),
        context: "discovery.mdns_save",
      });
      throw error;
    }
  }, [effective, mdnsDraft, t]);

  // 空态「开启 mDNS」入口：置草稿并滚动到 mDNS 卡，让改动位置可见
  const enableMdnsFromEmpty = useCallback(() => {
    setMdnsDraft(true);
    document
      .getElementById("discovery-mdns-card")
      ?.scrollIntoView({ behavior: "smooth", block: "center" });
  }, []);

  return (
    <>
      <PageHeader
        titleKey="discovery.title"
        descriptionKey="discovery.description"
      />
      {failed ? (
        <LoadFailedNotice onRetry={reload} messageKey="discovery.loadFailed" />
      ) : effective === null ? (
        <>
          <Skeleton className="col-span-12 h-40 lg:col-span-4" />
          <Skeleton className="col-span-12 h-40 lg:col-span-8" />
          <Skeleton className="col-span-12 h-64" />
        </>
      ) : (
        <>
          <MdnsCard
            config={effective}
            draft={mdnsDraft}
            running={running}
            onDraftChange={setMdnsDraft}
            onSave={saveMdns}
            onDiscard={() => setMdnsDraft(null)}
          />
          <RendezvousCard
            bootstrap={effective.bootstrap}
            onChange={persistBootstrap}
            addOpen={addOpen}
            onAddOpenChange={setAddOpen}
          />
          <DiscoveredTableCard
            mdnsActive={effective.enableMdns || mdnsDraft === true}
            onEnableMdns={enableMdnsFromEmpty}
            onAddAddress={() => setAddOpen(true)}
          />
        </>
      )}
    </>
  );
}
