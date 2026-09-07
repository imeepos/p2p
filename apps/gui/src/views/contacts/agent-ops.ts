import { useTranslation } from "react-i18next";

import { useAcpStore } from "@/acp/acp-store";
import { forgetEndpointMeta, useEndpointMetaStore } from "@/acp/endpoint-meta";
import type { AcpEndpoint } from "@/acp/protocol";
import { useConfirm } from "@/components/feedback/confirm-provider";
import { wsHostOf } from "@/lib/conversation-entry";

// Agent 停用/启用/删除流（通讯录行与资料卡共用）：§3.3 危险区语义——
// 停用第三档单次确认保留配置；删除第二档确认，红钮 + 后果明示。
// remove 返回错误文案（null = 成功），由调用方决定展示通道（行内错误条
// 或资料卡 toast），失败不静默。
export function useAgentOps() {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const removeSavedById = useAcpStore((s) => s.removeSavedById);

  const disable = async (endpoint: AcpEndpoint) => {
    const id = endpoint.endpointId!;
    const ok = await confirm({
      title: t("contacts.agents.disable"),
      description: t("contacts.agents.disabledHint"),
      confirmText: t("contacts.agents.disable"),
      cancelText: t("common.actions.cancel"),
      destructive: true,
    });
    if (ok) useEndpointMetaStore.getState().setDisabled(id, true);
  };

  const enable = (endpoint: AcpEndpoint) => {
    useEndpointMetaStore.getState().setDisabled(endpoint.endpointId!, false);
  };

  const remove = async (endpoint: AcpEndpoint): Promise<string | null> => {
    const id = endpoint.endpointId!;
    const ok = await confirm({
      title: t("contacts.agents.removeConfirmTitle"),
      description: t("contacts.agents.removeConfirmDescription", {
        name: endpoint.alias || wsHostOf(endpoint.wsUrl) || id,
      }),
      confirmText: t("contacts.agents.removeConfirmAction"),
      cancelText: t("common.actions.cancel"),
      destructive: true,
    });
    if (!ok) return null;
    try {
      removeSavedById(id);
      forgetEndpointMeta(id);
      return null;
    } catch (error) {
      console.error("[contacts] 删除 endpoint 失败", id, error);
      return error instanceof Error ? error.message : String(error);
    }
  };

  return { disable, enable, remove };
}
