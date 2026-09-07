import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Bot, MessageSquareIcon, Settings2Icon, Trash2Icon, UserRoundCheckIcon, UserRoundXIcon } from "lucide-react";

import { CopyButton } from "@/components/feedback/copy-button";
import { useAcpStore } from "@/acp/acp-store";
import { tierCounts } from "@/acp/endpoint-policy";
import { useEndpointMetaStore } from "@/acp/endpoint-meta";
import type { AcpEndpoint } from "@/acp/protocol";
import { wsHostOf } from "@/lib/conversation-entry";
import { toastError } from "@/components/feedback/toast";

import { AgentDetailDrawer } from "./agent-detail-drawer";
import { useAgentOps } from "./agent-ops";
import { ContactAvatar } from "./contact-avatar";
import {
  DetailAction,
  DetailActionLink,
  DetailActions,
  DetailRow,
  DetailRows,
  DetailShell,
} from "./detail-bits";

// Agent 资料卡：Bot 头像 + 别名；ID/host/连接态/权限摘要；底部发消息 /
// 详情 / 停用（或启用）/ 删除。详情抽屉与删除错误通道在卡内自持。
export function DetailAgent({ agent }: { agent: AcpEndpoint }) {
  const { t } = useTranslation();
  const phase = useAcpStore((s) => s.phase);
  const activeEndpointId = useAcpStore((s) => s.activeEndpointId);
  const disabledBy = useEndpointMetaStore((s) => s.disabled);
  const lastTestBy = useEndpointMetaStore((s) => s.lastTest);
  const policies = useEndpointMetaStore((s) => s.policies);
  const { disable, enable, remove } = useAgentOps();
  const [drawerId, setDrawerId] = useState<string | null>(null);

  const id = agent.endpointId ?? agent.wsUrl;
  const disabled = disabledBy[id] === true;
  const testFailed = lastTestBy[id] === "failed";
  const counts = tierCounts(policies[id] ?? { defaults: {}, exceptions: [] });
  const isActive = id === activeEndpointId;
  const dot = !isActive
    ? "bg-muted-foreground"
    : phase === "online"
      ? "bg-success"
      : phase === "offline"
        ? "bg-destructive"
        : "bg-warning";
  const name = agent.alias || wsHostOf(agent.wsUrl) || id;

  const onRemove = async () => {
    const error = await remove(agent);
    if (error) {
      toastError(t("contacts.agents.removeConfirmTitle"), { description: error });
    }
  };

  return (
    <DetailShell
      avatar={<ContactAvatar initial=" " className="size-16 rounded-lg" icon={<Bot aria-hidden className="size-8" />} />}
      title={<p className="truncate text-lg font-semibold">{name}</p>}
    >
      <DetailRows>
        <DetailRow label={t("contacts.detail.peerId")}>
          <span className="font-mono text-xs break-all">{id}</span>
          <CopyButton value={id} className="size-5 shrink-0" />
        </DetailRow>
        <DetailRow label={t("contacts.detail.status")}>
          <span aria-hidden data-testid={"contacts-detail-agent-dot-" + id} className={"size-2 shrink-0 rounded-full " + dot} />
          {disabled ? (
            <span className="bg-muted rounded px-1.5 py-0.5 text-xs" data-testid={"contacts-detail-agent-disabled-" + id}>
              {t("contacts.agents.disabledBadge")}
            </span>
          ) : null}
          {testFailed && !disabled ? (
            <span className="border-warning/50 text-warning rounded border px-1.5 py-0.5 text-xs">
              {t("contacts.agents.untestedBadge")}
            </span>
          ) : null}
        </DetailRow>
        <DetailRow label={t("contacts.drawer.policy")}>
          {t("contacts.agents.policySummary", { allow: counts.allow, ask: counts.ask, deny: counts.deny })}
        </DetailRow>
      </DetailRows>
      <DetailActions>
        {disabled ? (
          <DetailAction
            icon={MessageSquareIcon}
            label={t("contacts.agents.message")}
            testId="contacts-detail-message"
            disabled
            title={t("contacts.agents.messageDisabled")}
          />
        ) : (
          <DetailActionLink
            icon={MessageSquareIcon}
            label={t("contacts.agents.message")}
            testId="contacts-detail-message"
            to={"/chat?agent=" + id}
          />
        )}
        <DetailAction
          icon={Settings2Icon}
          label={t("contacts.agents.detail")}
          testId="contacts-detail-agent-detail"
          onClick={() => setDrawerId(id)}
        />
        {disabled ? (
          <DetailAction
            icon={UserRoundCheckIcon}
            label={t("contacts.agents.enable")}
            testId="contacts-detail-enable"
            onClick={() => enable(agent)}
          />
        ) : (
          <DetailAction
            icon={UserRoundXIcon}
            label={t("contacts.agents.disable")}
            testId="contacts-detail-disable"
            onClick={() => void disable(agent)}
          />
        )}
        <DetailAction
          icon={Trash2Icon}
          label={t("contacts.agents.removeConfirmAction")}
          testId="contacts-detail-remove"
          destructive
          onClick={() => void onRemove()}
        />
      </DetailActions>
      <AgentDetailDrawer endpointId={drawerId} onOpenChange={(open) => !open && setDrawerId(null)} />
    </DetailShell>
  );
}
