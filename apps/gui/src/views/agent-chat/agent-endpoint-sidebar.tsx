import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { Bot, Server } from "lucide-react";

import { useAcpStore } from "@/acp/acp-store";
import { cn } from "@/lib/utils";
import { EmptyState } from "@/views/shared/empty-state";

import { endpointRows, type AgentEndpointRow } from "./endpoint-rows";

// ACS1 侧栏（三区左区）容器：端点清单（本机 + 远端 saved 全量）为一级，
// 选中态路由化 ?endpoint=<id>——与对话区共用同一 query 主键，深链与点击同源。
// 端点下的会话清单与新建会话入口随下一提交接入。
function EndpointButton(props: {
  row: AgentEndpointRow;
  active: boolean;
  onSelect: (endpointId: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      onClick={() => props.onSelect(props.row.endpointId)}
      aria-current={props.active ? "true" : undefined}
      data-testid={"agent-endpoint-row-" + props.row.endpointId}
      className={cn(
        "flex w-full items-center gap-2 rounded-lg px-2 py-2 text-left transition-colors",
        props.active ? "bg-accent font-semibold" : "hover:bg-wx-hover",
      )}
    >
      {props.row.local ? (
        <Bot aria-hidden className="size-4 shrink-0" />
      ) : (
        <Server aria-hidden className="text-muted-foreground size-4 shrink-0" />
      )}
      <span className="min-w-0 flex-1 truncate text-sm">{props.row.label}</span>
      {props.row.local ? (
        <span className="text-muted-foreground shrink-0 text-[10px]">
          {t("agentChat.sidebar.localBadge")}
        </span>
      ) : null}
    </button>
  );
}

export function AgentEndpointSidebar({
  selectedEndpointId,
}: {
  selectedEndpointId: string | null;
}) {
  const { t } = useTranslation();
  const [searchParams, setSearchParams] = useSearchParams();
  const saved = useAcpStore((s) => s.saved);
  const consoleStatus = useAcpStore((s) => s.console);
  const unreadByEndpoint = useAcpStore((s) => s.unreadByEndpoint);
  const lastInteractionByEndpoint = useAcpStore((s) => s.lastInteractionByEndpoint);

  const rows = useMemo(
    () =>
      endpointRows({
        saved,
        localLabel: t("acp.console.localAgentName"),
        unreadByEndpoint,
        lastInteractionByEndpoint,
        includeLocalPlaceholder: consoleStatus !== null,
      }),
    [saved, t, unreadByEndpoint, lastInteractionByEndpoint, consoleStatus],
  );

  const select = (endpointId: string) => {
    const next = new URLSearchParams(searchParams);
    next.set("endpoint", endpointId);
    setSearchParams(next, { replace: true });
  };

  return (
    <section
      aria-label={t("agentChat.sidebar.title")}
      data-testid="agent-endpoint-sidebar"
      className="bg-wx-list flex min-h-0 w-[264px] shrink-0 flex-col border-r-[0.5px] border-border xl:w-[320px]"
    >
      {/* uix-spec §3 会话头：56px 折中高度 + 0.5px hairline */}
      <div className="flex h-14 shrink-0 items-center gap-2 border-b-[0.5px] border-border px-4 text-sm font-medium">
        <span className="min-w-0 flex-1 truncate">{t("agentChat.sidebar.title")}</span>
      </div>
      {rows.length === 0 ? (
        <div className="p-4">
          <EmptyState icon={Server} title={t("agentChat.sidebar.endpointsEmpty")} />
        </div>
      ) : (
        <nav
          aria-label={t("agentChat.sidebar.title")}
          data-testid="agent-endpoint-list"
          className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto p-2"
        >
          {rows.map((row) => (
            <EndpointButton
              key={row.endpointId}
              row={row}
              active={row.endpointId === selectedEndpointId}
              onSelect={select}
            />
          ))}
        </nav>
      )}
    </section>
  );
}
