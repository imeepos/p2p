import { useTranslation } from "react-i18next";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { ipc } from "@/lib/ipc";
import type { GuiConfig } from "@/lib/ipc-types";
import { errorText } from "@/views/shared/form-flow";
import { StatusBadge } from "@/views/shared/status-badge";

interface MdnsCardProps {
  config: GuiConfig | null;
  onSaved: (config: GuiConfig) => void;
}

// mDNS 状态卡：开关回读持久化配置，切换即保存（节点运行中需重启生效）。
export function MdnsCard({ config, onSaved }: MdnsCardProps) {
  const { t } = useTranslation();

  const toggle = async (next: boolean) => {
    if (!config) return;
    try {
      const saved = await ipc.configSave({ ...config, enableMdns: next });
      onSaved(saved);
      toastSuccess(t("discovery.mdns.saved"));
    } catch (error) {
      console.error("[discovery] mDNS 开关保存失败", error);
      toastError(t("discovery.mdns.saveFailed"), {
        description: errorText(error),
        context: "discovery.mdns_save",
      });
    }
  };

  return (
    <Card className="col-span-12 h-full lg:col-span-4">
      <CardHeader>
        <CardTitle>{t("discovery.mdns.title")}</CardTitle>
        <CardDescription>{t("discovery.mdns.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        {config === null ? (
          <Skeleton className="h-6 w-32" />
        ) : (
          <>
            <div className="flex items-center justify-between gap-4">
              <Label
                htmlFor="discovery-mdns-switch"
                className="flex items-center"
              >
                <StatusBadge
                  tone={config.enableMdns ? "success" : "neutral"}
                  dot
                >
                  {config.enableMdns
                    ? t("common.state.running")
                    : t("common.state.stopped")}
                </StatusBadge>
              </Label>
              <Switch
                id="discovery-mdns-switch"
                checked={config.enableMdns}
                onCheckedChange={(next) => void toggle(next)}
              />
            </div>
            {/* 状态详情占位：与右侧地址簿卡高度平衡，避免矮卡空洞 */}
            <p className="text-muted-foreground text-xs">
              {config.enableMdns
                ? t("discovery.mdns.runningDetail")
                : t("discovery.mdns.stoppedDetail")}
            </p>
          </>
        )}
      </CardContent>
    </Card>
  );
}
