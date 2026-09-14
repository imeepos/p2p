import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { Plus, Server } from "lucide-react";

import { useAcpStore } from "@/acp/acp-store";
import { newSessionAction } from "@/acp/new-session-action";
import { AsyncButton } from "@/components/feedback/async-button";
import { toastSuccess } from "@/components/feedback/toast";
import { EmptyState } from "@/views/shared/empty-state";

import { EndpointButton, EndpointSessions } from "./agent-endpoint-block";
import { endpointRows, sessionsOfEndpoint } from "./endpoint-rows";

// ACS1 侧栏（三区左区）容器：端点清单（本机 + 远端 saved 全量）为一级，会话清单
// 挂在已连接端点之下（store 的 sessions 只属于当前连接）。选中态路由化
// ?endpoint=<id>，与对话区共用同一 query 主键，深链与点击同源。
export function AgentEndpointSidebar({
  selectedEndpointId,
}: {
  selectedEndpointId: string | null;
}) {
  const { t } = useTranslation();
  const [searchParams, setSearchParams] = useSearchParams();
  const saved = useAcpStore((s) => s.saved);
  const consoleStatus = useAcpStore((s) => s.console);
  const phase = useAcpStore((s) => s.phase);
  const sessions = useAcpStore((s) => s.sessions);
  const activeEndpointId = useAcpStore((s) => s.activeEndpointId);
  const activeSessionId = useAcpStore((s) => s.activeSessionId);
  const newSessionPending = useAcpStore((s) => s.newSessionPending);
  const unreadByEndpoint = useAcpStore((s) => s.unreadByEndpoint);
  const lastInteractionByEndpoint = useAcpStore((s) => s.lastInteractionByEndpoint);
  const resumeSession = useAcpStore((s) => s.resumeSession);

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
            <div key={row.endpointId} className="flex flex-col gap-0.5">
              <EndpointButton
                row={row}
                active={row.endpointId === selectedEndpointId}
                onSelect={select}
              />
              {row.endpointId === selectedEndpointId ? (
                <EndpointSessions
                  endpointId={row.endpointId}
                  online={online && activeEndpointId === row.endpointId}
                  sessions={sessionsOfEndpoint({
                    sessions,
                    endpointId: row.endpointId,
                    activeEndpointId,
                    phase,
                  })}
                  activeSessionId={activeSessionId}
                  activeEndpointId={activeEndpointId}
                  lastInteractionByEndpoint={lastInteractionByEndpoint}
                  onOpen={(sessionId) => void resumeSession(sessionId)}
                />
              ) : null}
            </div>
          ))}
        </nav>
      )}
    </section>
  );
}
