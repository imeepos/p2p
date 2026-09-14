import { useTranslation } from "react-i18next";
import { Bot, Server } from "lucide-react";

import type { SessionSummary } from "@/acp/protocol";
import { cn } from "@/lib/utils";

import { AgentSessionRow } from "./agent-session-row";
import { sessionActivityMs, type AgentEndpointRow } from "./endpoint-rows";

// ACS1 侧栏端点块（展示层）：端点行 + 展开后的会话清单。数据由容器算好下发，
// 本文件只负责呈现与 testid，便于容器保持短小。
export function EndpointButton(props: {
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
      {props.row.unread > 0 ? (
        <span
          data-testid={"agent-endpoint-unread-" + props.row.endpointId}
          aria-label={t("agentChat.sidebar.unreadAria", { count: props.row.unread })}
          className="bg-destructive text-destructive-foreground shrink-0 rounded-full px-1.5 text-[10px] leading-4"
        >
          {props.row.unread > 99 ? "99+" : props.row.unread}
        </span>
      ) : null}
    </button>
  );
}

export function EndpointSessions(props: {
  endpointId: string;
  online: boolean;
  sessions: readonly SessionSummary[];
  activeSessionId: string | null;
  activeEndpointId: string | null;
  lastInteractionByEndpoint: Record<string, number>;
  onOpen: (sessionId: string) => void;
}) {
  const { t } = useTranslation();
  if (!props.online) {
    return (
      <p
        className="text-muted-foreground py-1 pr-2 pl-8 text-xs"
        data-testid="agent-sessions-offline"
      >
        {t("agentChat.sidebar.sessionsOffline")}
      </p>
    );
  }
  if (props.sessions.length === 0) {
    return (
      <p className="text-muted-foreground py-1 pr-2 pl-8 text-xs" data-testid="agent-sessions-empty">
        {t("agentChat.sidebar.sessionsEmpty")}
      </p>
    );
  }
  return (
    <div className="space-y-0.5" data-testid={"agent-session-list-" + props.endpointId}>
      {props.sessions.map((session) => (
        <AgentSessionRow
          key={session.sessionId}
          session={session}
          online={props.online}
          active={session.sessionId === props.activeSessionId}
          activityMs={sessionActivityMs({
            sessionId: session.sessionId,
            endpointId: props.endpointId,
            activeEndpointId: props.activeEndpointId,
            activeSessionId: props.activeSessionId,
            lastInteractionByEndpoint: props.lastInteractionByEndpoint,
          })}
          onOpen={props.onOpen}
        />
      ))}
    </div>
  );
}
