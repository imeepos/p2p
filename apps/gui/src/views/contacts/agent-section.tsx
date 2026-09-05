import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";
import { Bot, MessageSquareIcon, Settings2Icon, Trash2Icon, UserRoundXIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useAcpStore } from "@/acp/acp-store";
import {
  forgetEndpointMeta,
  useEndpointMetaStore,
} from "@/acp/endpoint-meta";
import { tierCounts } from "@/acp/endpoint-policy";
import type { AcpEndpoint } from "@/acp/protocol";
import { useConfirm } from "@/components/feedback/confirm-provider";
import { wsHostOf } from "@/lib/conversation-entry";
import { EmptyState } from "@/views/shared/empty-state";

import { EndpointAddDialog } from "./endpoint-add-dialog";
import { AgentDetailDrawer } from "./agent-detail-drawer";

// Agent 区（§3.1）：行 = 别名 + wsUrl host + 连接态 + 权限档摘要；行内
// 操作：发消息（/chat?agent=）、详情（右滑抽屉）、停用/删除（§3.3 危险区
// 语义：停用第三档单次确认保留配置；删除第二档，红钮 + 后果明示）。
export function AgentSection() {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const saved = useAcpStore((s) => s.saved);
  const phase = useAcpStore((s) => s.phase);
  const activeEndpointId = useAcpStore((s) => s.activeEndpointId);
  const removeSavedById = useAcpStore((s) => s.removeSavedById);
  const disabledBy = useEndpointMetaStore((s) => s.disabled);
  const lastTestBy = useEndpointMetaStore((s) => s.lastTest);
  const policies = useEndpointMetaStore((s) => s.policies);
  const [addOpen, setAddOpen] = useState(false);
  const [detailId, setDetailId] = useState<string | null>(null);
  const [commandError, setCommandError] = useState<string | null>(null);

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

  const remove = async (endpoint: AcpEndpoint) => {
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
    if (!ok) return;
    try {
      removeSavedById(id);
      forgetEndpointMeta(id);
    } catch (error) {
      console.error("[contacts] 删除 endpoint 失败", id, error);
      setCommandError(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <section
      id="agents"
      aria-label={t("contacts.section.agents")}
      data-testid="contacts-section-agents"
      className="bg-card ring-border ring-1 flex flex-col gap-2 rounded-lg p-4"
    >
      <div className="flex items-center justify-between gap-2">
        <h2 className="text-sm font-semibold">{t("contacts.section.agents")}</h2>
        <Button type="button" variant="outline" size="sm" onClick={() => setAddOpen(true)} data-testid="contacts-agent-add">
          <Bot aria-hidden className="size-4" />
          {t("contacts.agents.add")}
        </Button>
      </div>

      {commandError ? (
        <p className="text-destructive text-xs" role="alert" data-testid="contacts-agent-error">
          {commandError}
        </p>
      ) : null}

      {saved.length === 0 ? (
        <EmptyState
          icon={Bot}
          title={t("contacts.agents.empty")}
          description={t("contacts.agents.emptyHint")}
          action={
            <Button type="button" variant="outline" size="sm" onClick={() => setAddOpen(true)}>
              {t("contacts.agents.add")}
            </Button>
          }
        />
      ) : (
        saved.map((endpoint) => {
          const id = endpoint.endpointId ?? endpoint.wsUrl;
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
          const name = endpoint.alias || wsHostOf(endpoint.wsUrl) || id;
          return (
            <div
              key={id}
              className="hover:bg-accent/50 flex items-center gap-3 rounded-md px-2 py-2"
              data-testid={"contact-agent-" + id}
            >
              <span
                aria-hidden
                data-testid={"contact-agent-dot-" + id}
                className={"size-2 shrink-0 rounded-full " + dot}
              />
              <div className="min-w-0 flex-1">
                <p className="flex items-center gap-2 truncate text-sm font-medium">
                  {name}
                  {disabled ? (
                    <span className="bg-muted rounded px-1.5 py-0.5 text-xs" data-testid={"contact-agent-disabled-" + id}>
                      {t("contacts.agents.disabledBadge")}
                    </span>
                  ) : null}
                  {testFailed && !disabled ? (
                    <span
                      className="border-warning/50 text-warning rounded border px-1.5 py-0.5 text-xs"
                      data-testid={"contact-agent-warn-" + id}
                    >
                      {t("contacts.agents.untestedBadge")}
                    </span>
                  ) : null}
                </p>
                <p className="text-muted-foreground truncate text-xs">
                  {wsHostOf(endpoint.wsUrl) ?? endpoint.wsUrl} ·{" "}
                  {t("contacts.agents.policySummary", {
                    allow: counts.allow,
                    ask: counts.ask,
                    deny: counts.deny,
                  })}
                </p>
              </div>
              <div className="flex shrink-0 items-center gap-1">
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  asChild
                  disabled={disabled}
                >
                  <Link to={"/chat?agent=" + id} data-testid={"contact-agent-message-" + id} aria-disabled={disabled}>
                    <MessageSquareIcon aria-hidden className="size-4" />
                    {t("contacts.agents.message")}
                  </Link>
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() => setDetailId(id)}
                  data-testid={"contact-agent-detail-" + id}
                >
                  <Settings2Icon aria-hidden className="size-4" />
                  {t("contacts.agents.detail")}
                </Button>
                {disabled ? (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => enable(endpoint)}
                    data-testid={"contact-agent-enable-" + id}
                  >
                    {t("contacts.agents.enable")}
                  </Button>
                ) : (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => void disable(endpoint)}
                    data-testid={"contact-agent-disable-" + id}
                  >
                    <UserRoundXIcon aria-hidden className="size-4" />
                    {t("contacts.agents.disable")}
                  </Button>
                )}
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() => void remove(endpoint)}
                  data-testid={"contact-agent-remove-" + id}
                >
                  <Trash2Icon aria-hidden className="size-4" />
                  {t("contacts.friends.remove")}
                </Button>
              </div>
            </div>
          );
        })
      )}

      <EndpointAddDialog open={addOpen} onOpenChange={setAddOpen} onSaved={() => {}} />
      <AgentDetailDrawer endpointId={detailId} onOpenChange={(open) => !open && setDetailId(null)} />
    </section>
  );
}
