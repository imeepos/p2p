import { useTranslation } from "react-i18next";

import { AsyncButton } from "@/components/feedback/async-button";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import type { GuiConfig } from "@/lib/ipc-types";
import type { I18nKey } from "@/i18n/types";
import { StatusBadge } from "@/views/shared/status-badge";

interface MdnsCardProps {
  config: GuiConfig;
  /** 未保存草稿值；null 表示无草稿（展示值 = 持久化配置） */
  draft: boolean | null;
  running: boolean;
  onDraftChange: (next: boolean) => void;
  onSave: () => Promise<void>;
  onDiscard: () => void;
}

// 状态详情按生效时机分态：开关只写配置，节点重启才生效——不进行时声称正在广播。
function statusDetailKey(enabled: boolean, running: boolean): I18nKey {
  if (!enabled) return "discovery.mdns.stoppedDetail";
  return running
    ? "discovery.mdns.enabledRunningDetail"
    : "discovery.mdns.enabledNextStartDetail";
}

// mDNS 状态卡：与设置页统一「置脏 + 保存条」模型——切换只置脏，保存才落盘。
export function MdnsCard({
  config,
  draft,
  running,
  onDraftChange,
  onSave,
  onDiscard,
}: MdnsCardProps) {
  const { t } = useTranslation();
  const enabled = draft ?? config.enableMdns;

  return (
    <Card id="discovery-mdns-card" className="col-span-12 h-full lg:col-span-4">
      <CardHeader>
        <CardTitle>{t("discovery.mdns.title")}</CardTitle>
        <CardDescription>{t("discovery.mdns.hint")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <div className="flex items-center justify-between gap-4">
          <Label htmlFor="discovery-mdns-switch" className="flex items-center">
            <StatusBadge tone={enabled ? "success" : "neutral"} dot>
              {enabled
                ? t("discovery.mdns.enabledBadge")
                : t("discovery.mdns.disabledBadge")}
            </StatusBadge>
          </Label>
          <Switch
            id="discovery-mdns-switch"
            checked={enabled}
            onCheckedChange={onDraftChange}
          />
        </div>
        <p className="text-muted-foreground text-xs">
          {t(statusDetailKey(enabled, running))}
        </p>
        {draft !== null ? (
          <div className="border-warning/50 bg-warning/10 flex items-center justify-between gap-2 rounded-md border p-2 text-xs">
            <span className="flex-1">{t("discovery.mdns.draftNotice")}</span>
            <div className="flex gap-1.5">
              <Button type="button" variant="ghost" size="sm" onClick={onDiscard}>
                {t("discovery.mdns.discard")}
              </Button>
              <AsyncButton
                type="button"
                size="sm"
                action={onSave}
                loadingLabel={t("discovery.mdns.saving")}
              >
                {t("discovery.mdns.save")}
              </AsyncButton>
            </div>
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}
