import { useTranslation } from "react-i18next";
import { Bot, Loader2 } from "lucide-react";

import { useAcpStore } from "@/acp/acp-store";
import { Button } from "@/components/ui/button";
import { PromptComposer } from "@/acp/components/prompt-composer";
import { Transcript } from "@/acp/components/transcript";
import { EmptyState } from "@/views/shared/empty-state";

// agent 会话记录区（§2.1 右栏 agent 形态）：连接期复用 acp transcript/
// prompt-composer；未连接显连接引导卡（连接态与错误显式呈现，不静默）。

const PHASE_KEYS = {
  idle: "acp.connection.phase.idle",
  connecting: "acp.connection.phase.connecting",
  online: "acp.connection.phase.online",
  reconnecting: "acp.connection.phase.reconnecting",
  offline: "acp.connection.phase.offline",
} as const;
export function AgentConversation({ endpointId }: { endpointId: string }) {
  const { t } = useTranslation();
  const saved = useAcpStore((s) => s.saved);
  const phase = useAcpStore((s) => s.phase);
  const activeEndpointId = useAcpStore((s) => s.activeEndpointId);
  const activeSessionId = useAcpStore((s) => s.activeSessionId);
  const lastError = useAcpStore((s) => s.lastError);
  const closeInfo = useAcpStore((s) => s.closeInfo);
  const setDraft = useAcpStore((s) => s.setDraft);
  const connect = useAcpStore((s) => s.connect);
  const newSession = useAcpStore((s) => s.newSession);

  const endpoint = saved.find((e) => (e.endpointId ?? e.wsUrl) === endpointId);
  if (!endpoint) {
    return (
      <EmptyState
        className="max-w-none flex-1"
        icon={Bot}
        title={t("chat.agentPane.notFound")}
      />
    );
  }
  const title = endpoint.alias || endpointId;
  const connected = phase === "online" && activeEndpointId === endpointId;
  const connecting = phase === "connecting" && activeEndpointId === endpointId;

  if (connected) {
    return (
      <div data-testid="agent-conversation" className="flex min-h-0 flex-1 flex-col">
        <div className="shrink-0 border-b px-4 py-2 text-sm font-medium">
          <span className="inline-flex items-center gap-2">
            <Bot aria-hidden className="size-4" />
            {title}
          </span>
          <span className="text-muted-foreground ml-2 text-xs">{endpoint.wsUrl}</span>
        </div>
        {activeSessionId === null ? (
          <div className="flex flex-1 flex-col items-center justify-center gap-2">
            <p className="text-muted-foreground text-sm">{t("acp.sessions.empty")}</p>
            <Button type="button" size="sm" onClick={() => void newSession()} data-testid="agent-new-session">
              {t("chat.agentPane.newSession")}
            </Button>
          </div>
        ) : (
          <>
            <Transcript sessionId={activeSessionId} />
            <PromptComposer />
          </>
        )}
      </div>
    );
  }
  return (
    <div
      data-testid="agent-connect-card"
      className="flex flex-1 flex-col items-center justify-center gap-3 p-6"
    >
      <span className="inline-flex items-center gap-2 text-sm font-medium">
        <Bot aria-hidden className="size-4" />
        {title}
      </span>
      <span className="text-muted-foreground text-xs">{endpoint.wsUrl}</span>
      <span className="text-muted-foreground text-xs">{t(PHASE_KEYS[phase])}</span>
      {connecting ? (
        <span
          data-testid="agent-connecting"
          className="text-muted-foreground inline-flex items-center gap-2 text-sm"
        >
          <Loader2 aria-hidden className="size-4 animate-spin" />
          {t("acp.feedback.connecting")}
        </span>
      ) : (
        <Button
          type="button"
          size="sm"
          data-testid="agent-connect"
          onClick={() => {
            // 复用 console 连接管线：设草稿即聚焦该端点再拨号
            setDraft(endpoint);
            connect();
          }}
        >
          {t("chat.agentPane.connectAction")}
        </Button>
      )}
      {lastError || closeInfo ? (
        <p className="text-destructive text-xs" data-testid="agent-connect-error">
          {t("chat.agentPane.connectFailed")}
          {closeInfo ? ` (code=${closeInfo.code})` : ""}
          {lastError ? ` [${lastError}]` : ""}
        </p>
      ) : null}
    </div>
  );
}
