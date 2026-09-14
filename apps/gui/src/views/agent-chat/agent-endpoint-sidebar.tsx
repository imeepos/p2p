import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { Plus, Server } from "lucide-react";

import { useAcpStore } from "@/acp/acp-store";
import { newSessionAction } from "@/acp/new-session-action";
import { AsyncButton } from "@/components/feedback/async-button";
import { toastSuccess } from "@/components/feedback/toast";
import { EmptyState } from "@/views/shared/empty-state";
import type { AcpPhase, SessionSummary } from "@/acp/protocol";

import { EndpointButton, EndpointSessions } from "./agent-endpoint-block";
import { endpointRows, sessionsOfEndpoint, type AgentEndpointRow } from "./endpoint-rows";

// ACS1 侧栏（三区左区）容器：端点清单（本机 + 远端 saved 全量）为一级，会话清单
// 挂在已连接端点之下（store 的 sessions 只属于当前连接）。选中态路由化
// ?endpoint=<id>，与对话区共用同一 query 主键，深链与点击同源。
// ACS3-F1 补充项（ACS2 执行）：端点/会话清单抽为 EndpointList + EndpointRowItem，
// 容器函数回到 60 行红线内；store 订阅随清单内聚，渲染输出逐行等价。

/** 单端点行 + 命中选中态时内嵌会话清单 */
function EndpointRowItem(props: {
  row: AgentEndpointRow;
  selected: boolean;
  phase: AcpPhase;
  sessions: readonly SessionSummary[];
  activeEndpointId: string | null;
  activeSessionId: string | null;
  lastInteractionByEndpoint: Record<string, number>;
  resumeSession: (sessionId: string) => void;
  onSelect: (endpointId: string) => void;
}) {
  return (
    <div className="flex flex-col gap-0.5">
      <EndpointButton row={props.row} active={props.selected} onSelect={props.onSelect} />
      {props.selected ? (
        <EndpointSessions
          endpointId={props.row.endpointId}
          online={props.phase === "online" && props.activeEndpointId === props.row.endpointId}
          sessions={sessionsOfEndpoint({
            sessions: props.sessions,
            endpointId: props.row.endpointId,
            activeEndpointId: props.activeEndpointId,
            phase: props.phase,
          })}
          activeSessionId={props.activeSessionId}
          activeEndpointId={props.activeEndpointId}
          lastInteractionByEndpoint={props.lastInteractionByEndpoint}
          onOpen={(sessionId) => void props.resumeSession(sessionId)}
        />
      ) : null}
    </div>
  );
}

/** 端点/会话清单（含空态）：store 订阅内聚于此，容器只传选中态与选中回调 */
function EndpointList({
  selectedEndpointId,
  onSelect,
}: {
  selectedEndpointId: string | null;
  onSelect: (endpointId: string) => void;
}) {
  const { t } = useTranslation();
  const saved = useAcpStore((s) => s.saved);
  const consoleStatus = useAcpStore((s) => s.console);
  const phase = useAcpStore((s) => s.phase);
  const sessions = useAcpStore((s) => s.sessions);
  const activeEndpointId = useAcpStore((s) => s.activeEndpointId);
  const activeSessionId = useAcpStore((s) => s.activeSessionId);
  const unreadByEndpoint = useAcpStore((s) => s.unreadByEndpoint);
  const lastInteractionByEndpoint = useAcpStore((s) => s.lastInteractionByEndpoint);
  const resumeSession = useAcpStore((s) => s.resumeSession);

  const rows = useMemo(
    () => endpointRows({
      saved,
      localLabel: t("acp.console.localAgentName"),
      unreadByEndpoint,
      lastInteractionByEndpoint,
      includeLocalPlaceholder: consoleStatus !== null,
    }),
    [saved, t, unreadByEndpoint, lastInteractionByEndpoint, consoleStatus],
  );

  if (rows.length === 0) {
    return (
      <div className="p-4">
        <EmptyState icon={Server} title={t("agentChat.sidebar.endpointsEmpty")} />
      </div>
    );
  }
  return (
    <nav
      aria-label={t("agentChat.sidebar.title")}
      data-testid="agent-endpoint-list"
      className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto p-2"
    >
      {rows.map((row) => (
        <EndpointRowItem
          key={row.endpointId}
          row={row}
          selected={row.endpointId === selectedEndpointId}
          phase={phase}
          sessions={sessions}
          activeEndpointId={activeEndpointId}
          activeSessionId={activeSessionId}
          lastInteractionByEndpoint={lastInteractionByEndpoint}
          resumeSession={resumeSession}
          onSelect={onSelect}
        />
      ))}
    </nav>
  );
}

export function AgentEndpointSidebar({
  selectedEndpointId,
}: {
  selectedEndpointId: string | null;
}) {
  const { t } = useTranslation();
  const [searchParams, setSearchParams] = useSearchParams();
  const phase = useAcpStore((s) => s.phase);
  const activeEndpointId = useAcpStore((s) => s.activeEndpointId);
  const newSessionPending = useAcpStore((s) => s.newSessionPending);

  const select = (endpointId: string) => {
    const next = new URLSearchParams(searchParams);
    next.set("endpoint", endpointId);
    setSearchParams(next, { replace: true });
  };

  const online = phase === "online";
  // 新建会话只对当前连接端点有意义：选中非连接端点时不显按钮，避免误发到别的 agent
  const canCreate =
    online && selectedEndpointId !== null && selectedEndpointId === activeEndpointId;

  return (
    <section
      aria-label={t("agentChat.sidebar.title")}
      data-testid="agent-endpoint-sidebar"
      className="bg-wx-list flex min-h-0 w-[264px] shrink-0 flex-col border-r-[0.5px] border-border xl:w-[320px]"
    >
      {/* uix-spec §3 会话头：56px 折中高度 + 0.5px hairline */}
      <div className="flex h-14 shrink-0 items-center gap-2 border-b-[0.5px] border-border px-4 text-sm font-medium">
        <span className="min-w-0 flex-1 truncate">{t("agentChat.sidebar.title")}</span>
        {canCreate ? (
          <AsyncButton
            size="sm"
            variant="outline"
            disabled={newSessionPending}
            action={newSessionAction}
            onSuccess={() => toastSuccess(t("chat.feedback.sessionCreated"))}
            loadingLabel={t("chat.feedback.sessionCreating")}
            resultHoldMs={300}
            aria-label={t("agentChat.sidebar.newSession")}
            title={t("agentChat.sidebar.newSession")}
            data-testid="agent-sidebar-new-session"
          >
            <Plus aria-hidden className="size-4" />
          </AsyncButton>
        ) : null}
      </div>
      <EndpointList selectedEndpointId={selectedEndpointId} onSelect={select} />
    </section>
  );
}
