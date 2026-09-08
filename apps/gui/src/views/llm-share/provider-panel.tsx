import { Bot, RefreshCwIcon } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { useConfirm } from "@/components/feedback/confirm-provider";
import { toastError } from "@/components/feedback/toast";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "@/views/shared/empty-state";
import { errorText } from "@/views/shared/form-flow";

import {
  maskApiKey,
  migrateLegacyProvidersToStore,
  type ProviderConfig,
} from "./provider-configs";
import { ProviderEditForm } from "./provider-edit-form";
import { ShareCreateForm } from "./share-create-form";
import type { LlmProviderView, LlmServeStatus, LlmShareBackend } from "./types";

// 列表展示：后端掩码优先；本地新建未刷新的临时明文才走本地掩码
function displayApiKey(config: ProviderConfig): string {
  return config.apiKeyMasked ?? maskApiKey(config.apiKey);
}

function toViewConfig(view: LlmProviderView): ProviderConfig {
  return {
    id: view.id,
    name: view.name,
    baseUrl: view.baseUrl,
    protocol: view.protocol,
    apiKey: "",
    apiKeyMasked: view.apiKeyMasked,
    models: [...view.models],
    createdAt: view.createdAt,
  };
}

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
          <div className="flex items-center gap-2">
            <span className="truncate text-sm font-medium">{config.name}</span>
            <Badge variant="outline" className="text-[10px]">
              {t(
                config.protocol === "claude"
                  ? "llmShare.providers.protocolClaude"
                  : "llmShare.providers.protocolOpenai",
              )}
            </Badge>
          </div>
          <span className="text-muted-foreground truncate text-xs">{config.baseUrl}</span>
        </div>
        <span className="font-mono text-xs">{displayApiKey(config)}</span>
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
          data-testid="provider-create-share"
        >
          {t("llmShare.providers.createShareLink")}
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

function ServeStatusCard({
  status,
  onRefresh,
}: {
  status: LlmServeStatus;
  onRefresh: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Card data-testid="serve-status-card">
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle className="text-sm">{t("llmShare.providers.serveCardTitle")}</CardTitle>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          onClick={onRefresh}
          data-testid="serve-status-refresh"
        >
          <RefreshCwIcon aria-hidden className="size-3.5" />
          {t("llmShare.serve.refresh")}
        </Button>
      </CardHeader>
      <CardContent className="flex flex-col gap-1">
        {status.assembled ? (
          <p className="text-xs" data-testid="serve-assembled">
            {t("llmShare.serve.assembled")}
          </p>
        ) : (
          <p className="text-xs" data-testid="serve-not-assembled">
            {t("llmShare.serve.notAssembled")}
          </p>
        )}
        {status.models.length > 0 ? (
          <p className="text-muted-foreground text-xs">{status.models.join(", ")}</p>
        ) : null}
        {status.lastError ? (
          <p role="alert" className="text-destructive text-xs" data-testid="serve-last-error">
            {t("llmShare.serve.lastError", { error: status.lastError })}
          </p>
        ) : null}
      </CardContent>
    </Card>
  );
}

// 上游配置面板（契约 §16.6 v13）：存储归属后端 ProviderStore——挂载时一次性幂等迁移
// 旧 localStorage 存档，之后增删改全走 providerList/Save/Remove；apiKey 只出掩码。
// 每条配置可「生成分享链接」（dsh-llm-share://），serve 装配状态卡常驻展示。
export function ProviderPanel({ backend }: { backend: LlmShareBackend }) {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const [configs, setConfigs] = useState<ProviderConfig[]>([]);
  const [serve, setServe] = useState<LlmServeStatus | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [editing, setEditing] = useState<ProviderConfig | null | undefined>(undefined);
  const [shareForId, setShareForId] = useState<string | null>(null);
  const formOpen = editing !== undefined;

  const refresh = useCallback(async () => {
    const [list, status] = await Promise.all([backend.providerList(), backend.serveStatus()]);
    setConfigs(list.providers.map(toViewConfig));
    setServe(status);
  }, [backend]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        // 迁移幂等：无旧键即 no-op；成功后写路径不再落明文键
        await migrateLegacyProvidersToStore(backend);
        const [list, status] = await Promise.all([backend.providerList(), backend.serveStatus()]);
        if (cancelled) return;
        setConfigs(list.providers.map(toViewConfig));
        setServe(status);
      } catch (error) {
        if (cancelled) return;
        console.error("[llm-share] provider 列表加载失败", error);
        setLoadError(errorText(error));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [backend]);

  const handleSave = async (config: ProviderConfig) => {
    try {
      // apiKey 留空 = 保留原密钥（编辑态）；仅新增/改钥传明文
      await backend.providerSave({
        id: config.id,
        name: config.name,
        baseUrl: config.baseUrl,
        protocol: config.protocol,
        apiKey: config.apiKey || undefined,
        models: config.models,
      });
      setEditing(undefined);
      await refresh();
    } catch (error) {
      console.error("[llm-share] provider 保存失败", error);
      toastError(t("llmShare.providers.saveFailed"), { description: errorText(error) });
    }
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
      try {
        await backend.providerRemove(config.id);
        if (shareForId === config.id) setShareForId(null);
        await refresh();
      } catch (error) {
        console.error("[llm-share] provider 删除失败", error);
        toastError(t("llmShare.providers.removeFailed"), { description: errorText(error) });
      }
    })();
  };

  return (
    <div className="flex flex-col gap-3" data-testid="provider-panel">
      {serve ? <ServeStatusCard status={serve} onRefresh={() => void refresh()} /> : null}
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
              onSave={(config) => void handleSave(config)}
              onCancel={() => setEditing(undefined)}
            />
          ) : (
            <p className="text-muted-foreground text-xs">
              {loadError ?? t("llmShare.providers.localHint")}
            </p>
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
              <ShareCreateForm
                provider={config}
                backend={backend}
                onCreated={() => setShareForId(null)}
                onCancel={() => setShareForId(null)}
              />
            ) : null}
          </div>
        ))
      )}
    </div>
  );
}
