import { Bot } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { useConfirm } from "@/components/feedback/confirm-provider";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/views/shared/empty-state";

import {
  loadProviderConfigs,
  maskApiKey,
  removeProviderConfig,
  saveProviderConfigs,
  upsertProviderConfig,
  type ProviderConfig,
} from "./provider-configs";
import { ProviderEditForm } from "./provider-edit-form";
import { ProviderShareForm } from "./provider-share-form";
import type { LlmShareBackend } from "./types";

function ConfigRow({
  config,
  sharing,
  onShare,
  onEdit,
  onRemove,
}: {
  config: ProviderConfig;
  sharing: boolean;
  onShare: () => void;
  onEdit: () => void;
  onRemove: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-2 rounded-md border p-3" data-testid="provider-row">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 flex-col">
          <span className="truncate text-sm font-medium">{config.name}</span>
          <span className="text-muted-foreground truncate text-xs">{config.baseUrl}</span>
        </div>
        <span className="font-mono text-xs">{maskApiKey(config.apiKey)}</span>
      </div>
      <div className="flex flex-wrap gap-1">
        {config.models.map((model) => (
          <Badge key={model} variant="secondary" className="text-xs">
            {model}
          </Badge>
        ))}
      </div>
      <div className="flex gap-2">
        <Button
          type="button"
          size="sm"
          variant={sharing ? "secondary" : "default"}
          onClick={onShare}
          data-testid="provider-share-toggle"
        >
          {t("llmShare.providers.shareWith")}
        </Button>
        <Button type="button" size="sm" variant="outline" onClick={onEdit}>
          {t("llmShare.providers.edit")}
        </Button>
        <Button type="button" size="sm" variant="outline" onClick={onRemove}>
          {t("llmShare.providers.remove")}
        </Button>
      </div>
    </div>
  );
}

// 上游配置面板：本地自用的 provider 配置列表（localStorage 存档，密钥不出本机）。
// 每条配置可一键分享给好友：offerPublish（按配置模型发布能力声明）+
// allow（好友 PeerId 按同批模型放行）——分享的是可用性，不是密钥本体。
export function ProviderPanel({ backend }: { backend: LlmShareBackend }) {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const [configs, setConfigs] = useState<ProviderConfig[]>(loadProviderConfigs);
  // undefined = 列表态；null = 新增；ProviderConfig = 编辑该条
  const [editing, setEditing] = useState<ProviderConfig | null | undefined>(undefined);
  const [shareForId, setShareForId] = useState<string | null>(null);
  const formOpen = editing !== undefined;

  const persist = (next: ProviderConfig[]) => {
    setConfigs(next);
    saveProviderConfigs(next);
  };

  const handleSave = (config: ProviderConfig) => {
    persist(upsertProviderConfig(configs, config));
    setEditing(undefined);
  };

  const handleRemove = (config: ProviderConfig) => {
    void (async () => {
      const ok = await confirm({
        title: t("llmShare.providers.removeConfirmTitle"),
        description: t("llmShare.providers.removeConfirmDesc", { name: config.name }),
        confirmText: t("llmShare.providers.remove"),
        cancelText: t("common.actions.cancel"),
        destructive: true,
      });
      if (!ok) return;
      persist(removeProviderConfig(configs, config.id));
      if (shareForId === config.id) setShareForId(null);
    })();
  };

  return (
    <div className="flex flex-col gap-3" data-testid="provider-panel">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle className="text-sm">{t("llmShare.providers.cardTitle")}</CardTitle>
          {!formOpen ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => setEditing(null)}
              data-testid="provider-add"
            >
              {t("llmShare.providers.addProvider")}
            </Button>
          ) : null}
        </CardHeader>
        <CardContent>
          {formOpen ? (
            <ProviderEditForm
              editing={editing === null ? null : editing}
              onSave={handleSave}
              onCancel={() => setEditing(undefined)}
            />
          ) : (
            <p className="text-muted-foreground text-xs">{t("llmShare.providers.localHint")}</p>
          )}
        </CardContent>
      </Card>
      {configs.length === 0 && !formOpen ? (
        <EmptyState
          icon={Bot}
          title={t("llmShare.providers.emptyTitle")}
          description={t("llmShare.providers.emptyHint")}
        />
      ) : (
        configs.map((config) => (
          <div key={config.id} className="flex flex-col gap-2">
            <ConfigRow
              config={config}
              sharing={shareForId === config.id}
              onShare={() => setShareForId(shareForId === config.id ? null : config.id)}
              onEdit={() => {
                setShareForId(null);
                setEditing(config);
              }}
              onRemove={() => handleRemove(config)}
            />
            {shareForId === config.id ? (
              <ProviderShareForm
                config={config}
                backend={backend}
                onShared={() => setShareForId(null)}
                onCancel={() => setShareForId(null)}
              />
            ) : null}
          </div>
        ))
      )}
    </div>
  );
}
