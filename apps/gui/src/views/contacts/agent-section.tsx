import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link, useSearchParams } from "react-router-dom";
import { Bot, MessageSquareIcon, Settings2Icon, Trash2Icon, UserRoundCheckIcon, UserRoundXIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { CopyButton } from "@/components/feedback/copy-button";
import { useAcpStore } from "@/acp/acp-store";
import { tierCounts } from "@/acp/endpoint-policy";
import { useEndpointMetaStore } from "@/acp/endpoint-meta";
import type { AcpEndpoint } from "@/acp/protocol";
import { wsHostOf } from "@/lib/conversation-entry";
import { EmptyState } from "@/views/shared/empty-state";
import { cn } from "@/lib/utils";

import { EndpointAddDialog } from "./endpoint-add-dialog";
import { AgentDetailDrawer } from "./agent-detail-drawer";
import { useAgentOps } from "./agent-ops";
import { CONTACT_ROW_CLS, ContactAvatar, ROW_ACTIONS_CLS } from "./contact-avatar";
import { selectionKey } from "./contacts-detail-model";
import { matchesQuery, useContactsPane } from "./contacts-sections";
import { TreeSection } from "./contacts-tree";

// Agent 行（§3.1，双栏改版）：点选联动右栏资料卡；行内动作（发消息/详情/
// 停用/删除）悬停显隐，仍由节内抽屉/确认/错误条承接。
function AgentRow(props: {
  endpoint: AcpEndpoint;
  onDetail: (id: string) => void;
  onDisable: (endpoint: AcpEndpoint) => void;
  onEnable: (endpoint: AcpEndpoint) => void;
  onRemove: (endpoint: AcpEndpoint) => void;
}) {
  const { t } = useTranslation();
  const pane = useContactsPane();
  const phase = useAcpStore((s) => s.phase);
  const activeEndpointId = useAcpStore((s) => s.activeEndpointId);
  const disabledBy = useEndpointMetaStore((s) => s.disabled);
  const lastTestBy = useEndpointMetaStore((s) => s.lastTest);
  const policies = useEndpointMetaStore((s) => s.policies);
  const { endpoint, onDetail, onDisable, onEnable, onRemove } = props;
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
  const selected = pane.selectedKey === selectionKey({ kind: "agent", endpointId: id });
  return (
    <div
      className={cn(CONTACT_ROW_CLS, selected ? "bg-accent" : "hover:bg-accent/60")}
      data-testid={"contact-agent-" + id}
    >
      <button
        type="button"
        className="flex min-w-0 flex-1 items-center gap-2.5 rounded-md text-left"
        onClick={() => pane.select({ kind: "agent", endpointId: id })}
      >
        <ContactAvatar initial={name.slice(0, 1)} icon={<Bot aria-hidden className="size-4" />} />
        <span className="min-w-0 flex-1">
          <span className="flex items-center gap-2 text-sm font-medium">
            <span className="truncate">{name}</span>
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
          </span>
          <span className="text-muted-foreground block truncate text-xs">
            {wsHostOf(endpoint.wsUrl) ?? endpoint.wsUrl} ·{" "}
            {t("contacts.agents.policySummary", {
              allow: counts.allow,
              ask: counts.ask,
              deny: counts.deny,
            })}
          </span>
        </span>
      </button>
      <span
        aria-hidden
        data-testid={"contact-agent-dot-" + id}
        className={cn("size-2 shrink-0 rounded-full", dot)}
      />
      <div className={ROW_ACTIONS_CLS}>
        {disabled ? (
          <span
            aria-disabled
            title={t("contacts.agents.messageDisabled")}
            className="text-muted-foreground inline-flex size-7 items-center justify-center rounded-md opacity-60"
            data-testid={"contact-agent-message-" + id}
          >
            <MessageSquareIcon aria-hidden className="size-4" />
          </span>
        ) : (
          <Button type="button" variant="ghost" size="icon" className="size-7" asChild>
            <Link
              to={"/chat?agent=" + id}
              data-testid={"contact-agent-message-" + id}
              title={t("contacts.agents.message")}
              aria-label={t("contacts.agents.message")}
            >
              <MessageSquareIcon aria-hidden className="size-4" />
            </Link>
          </Button>
        )}
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-7"
          onClick={() => onDetail(id)}
          data-testid={"contact-agent-detail-" + id}
          title={t("contacts.agents.detail")}
          aria-label={t("contacts.agents.detail")}
        >
          <Settings2Icon aria-hidden className="size-4" />
        </Button>
        {disabled ? (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="size-7"
            onClick={() => onEnable(endpoint)}
            data-testid={"contact-agent-enable-" + id}
            title={t("contacts.agents.enable")}
            aria-label={t("contacts.agents.enable")}
          >
            <UserRoundCheckIcon aria-hidden className="size-4" />
          </Button>
        ) : (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="size-7"
            onClick={() => onDisable(endpoint)}
            data-testid={"contact-agent-disable-" + id}
            title={t("contacts.agents.disable")}
            aria-label={t("contacts.agents.disable")}
          >
            <UserRoundXIcon aria-hidden className="size-4" />
          </Button>
        )}
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-7"
          onClick={() => onRemove(endpoint)}
          data-testid={"contact-agent-remove-" + id}
          title={t("contacts.agents.removeConfirmAction")}
          aria-label={t("contacts.agents.removeConfirmAction")}
        >
          <Trash2Icon aria-hidden className="size-4" />
        </Button>
      </div>
    </div>
  );
}

// Agent 节（§3.1，双栏改版）：行 = 别名 + host + 连接态 + 权限档摘要；
// R2-23/R2-24 深链 /contacts?agentDetail=<id> 直开详情抽屉保留在本节。
export function AgentSection() {
  const { t } = useTranslation();
  const pane = useContactsPane();
  const saved = useAcpStore((s) => s.saved);
  const [addOpen, setAddOpen] = useState(false);
  const [commandError, setCommandError] = useState<string | null>(null);
  const [searchParams] = useSearchParams();
  const { disable, enable, remove } = useAgentOps();

  // 深链只读参数：命中才开抽屉，未命中留 warn 观测信号，不静默。渲染期
  // 同步（react-hooks 纪律，同 contacts-view hash 深链先例），不放 effect。
  const deepLinkAgentId = searchParams.get("agentDetail");
  const resolveDeepLink = (id: string | null): string | null => {
    if (!id) return null;
    const known = saved.some((e) => (e.endpointId ?? e.wsUrl) === id);
    if (!known) console.warn("[contacts] agentDetail 深链未命中已登记端点: " + id);
    return known ? id : null;
  };
  const [detailId, setDetailId] = useState<string | null>(() =>
    resolveDeepLink(deepLinkAgentId),
  );
  const [lastDeepLink, setLastDeepLink] = useState(deepLinkAgentId);
  if (deepLinkAgentId !== lastDeepLink) {
    setLastDeepLink(deepLinkAgentId);
    setDetailId(resolveDeepLink(deepLinkAgentId));
  }

  const onRemove = async (endpoint: AcpEndpoint) => {
    const error = await remove(endpoint);
    if (error) setCommandError(error);
  };

  // P2#8 检索：别名/host/PeerId/endpointId/wsUrl 子串匹配，大小写不敏感
  const filtered = saved.filter((endpoint) =>
    matchesQuery(
      [endpoint.alias, wsHostOf(endpoint.wsUrl), endpoint.endpointId, endpoint.peer, endpoint.wsUrl],
      pane.query,
    ),
  );

  return (
    <TreeSection
      id="agents"
      wrapperTestId="contacts-section-agents"
      title={t("contacts.section.agents")}
      expanded={!pane.isSectionCollapsed("agents")}
      onToggle={() => pane.toggleSection("agents")}
      toggleTestId="contacts-tree-toggle-agents"
      active={pane.activeSection === "agents"}
      onGo={() => pane.gotoSection("agents")}
      anchorTestId="contacts-anchor-agents"
      anchorLabel={t("contacts.anchor.goto", { section: t("contacts.section.agents") })}
      count={
        <span className="text-muted-foreground shrink-0 text-xs" data-testid="contacts-count-agents">
          {t("contacts.countOf", { matched: filtered.length, total: saved.length })}
        </span>
      }
      actions={
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-6"
          onClick={() => setAddOpen(true)}
          data-testid="contacts-agent-add"
          title={t("contacts.agents.add")}
          aria-label={t("contacts.agents.add")}
        >
          <Bot aria-hidden className="size-4" />
        </Button>
      }
    >
      {commandError ? (
        <div className="flex items-center gap-1" data-testid="contacts-agent-error-row">
          <p className="text-destructive text-xs" role="alert" data-testid="contacts-agent-error">
            {commandError}
          </p>
          <CopyButton value={commandError} className="size-5" />
        </div>
      ) : null}

      {saved.length === 0 ? (
        <EmptyState
          icon={Bot}
          title={t("contacts.agents.empty")}
          description={t("contacts.agents.emptyHint")}
        />
      ) : (
        filtered.map((endpoint) => (
          <AgentRow
            key={endpoint.endpointId ?? endpoint.wsUrl}
            endpoint={endpoint}
            onDetail={setDetailId}
            onDisable={(e) => void disable(e)}
            onEnable={enable}
            onRemove={(e) => void onRemove(e)}
          />
        ))
      )}

      <EndpointAddDialog open={addOpen} onOpenChange={setAddOpen} onSaved={() => {}} />
      <AgentDetailDrawer endpointId={detailId} onOpenChange={(open) => !open && setDetailId(null)} />
    </TreeSection>
  );
}
