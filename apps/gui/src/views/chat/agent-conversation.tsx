import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";
import { Bot, Loader2 } from "lucide-react";

import { useAcpStore } from "@/acp/acp-store";
import { LOCAL_AGENT_ENDPOINT_ID } from "@/acp/console-client";
import { Button } from "@/components/ui/button";
import { PromptComposer } from "@/acp/components/prompt-composer";
import { Transcript } from "@/acp/components/transcript";
import { EmptyState } from "@/views/shared/empty-state";
import type { AcpConsoleStatus } from "@/lib/ipc-types";
import type { I18nKey } from "@/i18n/types";
// 模块加载即订阅 acp-console 托管事件（幂等；chat 路由静态链保证应用启动即生效）
import "@/acp/console-watch";

// agent 会话记录区（§2.1 右栏 agent 形态）：连接期复用 acp transcript/
// prompt-composer；未连接显连接引导卡（连接态与错误显式呈现，不静默）。
// UX3：本机 agent 端点在 console 非 ready 相位（failed/unavailable/starting/
// restarting/stopped）显引导卡——安装/日志指引与相位状态，绝不静默。

const PHASE_KEYS = {
  idle: "acp.connection.phase.idle",
  connecting: "acp.connection.phase.connecting",
  online: "acp.connection.phase.online",
  reconnecting: "acp.connection.phase.reconnecting",
  offline: "acp.connection.phase.offline",
} as const;

function consolePhaseKey(phase: string): I18nKey {
  return ("acp.console.phase." + phase) as I18nKey;
}

/** console 非 ready 的显式引导卡：相位 + 安装/日志指引（failed/unavailable）
 *  或启动/重启进行时提示；lastError 透出，附诊断页入口 */
function ConsoleGuideCard({ status }: { status: AcpConsoleStatus }) {
  const { t } = useTranslation();
  const broken = status.phase === "failed" || status.phase === "unavailable";
  const hintKey = broken
    ? status.phase === "failed"
      ? "acp.console.guide.failedHint"
      : "acp.console.guide.unavailableHint"
    : status.phase === "restarting"
      ? "acp.console.guide.restartingHint"
      : "acp.console.guide.startingHint";
  return (
    <div
      data-testid="agent-console-guide"
      className="flex flex-1 flex-col items-center justify-center gap-3 p-6"
    >
      <span className="inline-flex items-center gap-2 text-sm font-medium">
        <Bot aria-hidden className="size-4" />
        {t("acp.console.localAgentName")}
      </span>
      <span className="text-muted-foreground text-xs" data-testid="agent-console-phase">
        {t(consolePhaseKey(status.phase), { restarts: status.restarts })}
      </span>
      <p className="text-muted-foreground max-w-md text-center text-xs" data-testid="agent-console-guide-hint">
        {t(hintKey)}
      </p>
      {broken && status.lastError ? (
        <p className="text-destructive text-xs" data-testid="agent-console-guide-error">
          {t("acp.console.guide.lastError", { error: status.lastError })}
        </p>
      ) : null}
      {broken ? (
        <Button asChild size="sm" variant="outline" data-testid="agent-console-guide-logs">
          <Link to="/diagnostics">{t("acp.console.guide.logsAction")}</Link>
        </Button>
      ) : null}
    </div>
  );
}

export function AgentConversation({ endpointId }: { endpointId: string }) {
  const { t } = useTranslation();
  const saved = useAcpStore((s) => s.saved);
  const phase = useAcpStore((s) => s.phase);
  const activeEndpointId = useAcpStore((s) => s.activeEndpointId);
  const activeSessionId = useAcpStore((s) => s.activeSessionId);
  const lastError = useAcpStore((s) => s.lastError);
  const closeInfo = useAcpStore((s) => s.closeInfo);
  const consoleStatus = useAcpStore((s) => s.console);
  const flowNotice = useAcpStore((s) => s.consoleFlowNotice);
  const setDraft = useAcpStore((s) => s.setDraft);
  const connect = useAcpStore((s) => s.connect);
  const newSession = useAcpStore((s) => s.newSession);

  const isLocal = endpointId === LOCAL_AGENT_ENDPOINT_ID;
  const consoleDown = consoleStatus !== null && consoleStatus.phase !== "ready";

  const endpoint = saved.find((e) => (e.endpointId ?? e.wsUrl) === endpointId);
  if (!endpoint) {
    // 端点未登记且 console 异常：以引导卡解释根因，而非裸 notFound
    if (consoleDown && consoleStatus) return <ConsoleGuideCard status={consoleStatus} />;
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
  // 本机 agent + console 非 ready：引导卡接管（不提供注定失败的手动连接按钮）
  if (isLocal && consoleDown && consoleStatus) {
    return (
      <div className="flex min-h-0 flex-1 flex-col">
        <ConsoleGuideCard status={consoleStatus} />
        {flowNotice ? (
          <p className="text-muted-foreground pb-4 text-center text-xs" data-testid="agent-console-notice">
            {t(flowNotice as I18nKey)}
          </p>
        ) : null}
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
      {flowNotice ? (
        <p className="text-muted-foreground text-xs" data-testid="agent-console-notice">
          {t(flowNotice as I18nKey)}
        </p>
      ) : null}
      {lastError || closeInfo ? (
        <p className="text-destructive text-xs" data-testid="agent-connect-error">
          {t("chat.agentPane.connectFailed")}
          {closeInfo ? " (code=" + closeInfo.code + ")" : ""}
          {lastError ? " [" + lastError + "]" : ""}
        </p>
      ) : null}
    </div>
  );
}