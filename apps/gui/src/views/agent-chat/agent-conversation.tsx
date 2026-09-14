import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";
import { Bot, Loader2, TriangleAlertIcon } from "lucide-react";

import { useAcpStore } from "@/acp/acp-store";
import { newSessionAction } from "@/acp/new-session-action";
import { acpErrorDetail, connectFailureText } from "@/acp/error-help";
import { LOCAL_AGENT_ENDPOINT_ID } from "@/acp/console-client";
import type { AcpCloseInfo } from "@/acp/protocol";
import { AsyncButton } from "@/components/feedback/async-button";
import { CopyButton } from "@/components/feedback/copy-button";
import { toastSuccess } from "@/components/feedback/toast";
import { Button } from "@/components/ui/button";
import { PromptComposer } from "@/acp/components/prompt-composer";
import { Transcript } from "@/acp/components/transcript";
import { EmptyState } from "@/views/shared/empty-state";
import type { AcpConsoleStatus } from "@/lib/ipc-types";
import { wsHostOf } from "@/lib/conversation-entry";
import type { I18nKey } from "@/i18n/types";
// 模块加载即订阅 acp-console 托管事件（幂等；agent 页路由静态链保证应用启动即生效）
import "@/acp/console-watch";
// 模块加载即注册 VITE_MOCK_IPC=1 走查注入入口（生产构建不暴露）
import "./agent-conversation-inject";

// agent 会话记录区（/agent 页对话区，ACS2 自 /chat 迁入，行为零改动）：
// 连接期复用 acp transcript/prompt-composer；未连接显连接引导卡（连接态与
// 错误显式呈现，不静默）。
// INLINE-ACP-PUMP 起 console 为 in-process 泵三态（connecting/connected/
// disconnected）：disconnected 显失败留痕与日志指引，connecting 显进行时，
// 绝不静默。

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

/** console 非 connected 的显式引导卡：相位 + 进行时/断开提示；lastError 透出，
 *  附诊断页入口 */
function ConsoleGuideCard({ status }: { status: AcpConsoleStatus }) {
  const { t } = useTranslation();
  const down = status.phase === "disconnected";
  const hintKey = down
    ? "acp.console.guide.disconnectedHint"
    : "acp.console.guide.connectingHint";
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
        {t(consolePhaseKey(status.phase))}
      </span>
      <p className="text-muted-foreground max-w-md text-center text-xs" data-testid="agent-console-guide-hint">
        {t(hintKey)}
      </p>
      {down && status.lastError ? (
        <p className="text-destructive text-xs" data-testid="agent-console-guide-error">
          {t("acp.console.guide.lastError", { error: status.lastError })}
        </p>
      ) : null}
      {down ? (
        <Button asChild size="sm" variant="outline" data-testid="agent-console-guide-logs">
          <Link to="/diagnostics">{t("acp.console.guide.logsAction")}</Link>
        </Button>
      ) : null}
    </div>
  );
}

// F06：连接失败行内出「原因 + 下一步动作」人话；内部码收进可复制详情。
// R2-23：指引指向真实编辑路径并给通讯录编辑抽屉直达（agentDetail 深链）
function ConnectFailureNotice({ lastError, closeInfo, wsUrl, endpointId }: {
  lastError: string | null;
  closeInfo: AcpCloseInfo | null;
  wsUrl: string;
  endpointId: string;
}) {
  const { t } = useTranslation();
  const detail = acpErrorDetail({ lastError, closeInfo, wsUrl });
  const reason = connectFailureText(t, lastError, closeInfo);
  return (
    <div className="flex flex-col items-center gap-1" data-testid="agent-connect-error">
      <p className="text-destructive text-xs" data-testid="agent-connect-error-text">
        {t("chat.agentPane.connectFailed")}：{reason}
      </p>
      {detail ? (
        <CopyButton
          value={detail}
          className="size-5"
          aria-label={t("common.feedback.copyDetail")}
          title={detail}
          data-testid="agent-connect-error-copy"
        />
      ) : null}
      <Button asChild size="sm" variant="outline" data-testid="agent-edit-link">
        <Link to={"/contacts?agentDetail=" + encodeURIComponent(endpointId)}>
          {t("chat.agentPane.editInContacts")}
        </Link>
      </Button>
    </div>
  );
}

// R2-24（产品裁决：最小可见性）：存在待应答权限请求时出指示条 + 直达
// contacts Agent 详情权限面板（agentDetail 深链）；无待应答零痕迹。
// 计数读 store 已消费的 interactions（事件管线注册的 pending 项）。
function PendingPermissionBanner({ endpointId }: { endpointId: string }) {
  const { t } = useTranslation();
  const pendingCount = useAcpStore((s) =>
    Object.values(s.interactions).reduce(
      (n, i) => n + i.permissions.filter((p) => p.status === "pending").length,
      0,
    ),
  );
  if (pendingCount === 0) return null;
  return (
    <div
      role="status"
      data-testid="agent-permission-banner"
      className="border-warning/50 bg-warning/10 text-warning mx-4 mt-2 flex shrink-0 items-center gap-2 rounded-md border-[0.5px] px-3 py-1.5 text-xs"
    >
      <TriangleAlertIcon aria-hidden className="size-3.5 shrink-0" />
      <span className="min-w-0 flex-1 truncate" data-testid="agent-permission-banner-text">
        {t("chat.agentPane.permissionPending", { count: pendingCount })}
      </span>
      <Button asChild size="sm" variant="outline" data-testid="agent-permission-banner-action">
        <Link to={"/contacts?agentDetail=" + encodeURIComponent(endpointId)}>
          {t("chat.agentPane.permissionPendingAction")}
        </Link>
      </Button>
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
  const newSessionPending = useAcpStore((s) => s.newSessionPending);

  const isLocal = endpointId === LOCAL_AGENT_ENDPOINT_ID;
  const consoleDown = consoleStatus !== null && consoleStatus.phase !== "connected";

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
  // P3#16 标题兜底：无别名时用 wsUrl host，最后才是裸 endpointId
  const title = endpoint.alias || wsHostOf(endpoint.wsUrl) || endpointId;
  const connected = phase === "online" && activeEndpointId === endpointId;
  const connecting = phase === "connecting" && activeEndpointId === endpointId;

  if (connected) {
    return (
      <div data-testid="agent-conversation" className="flex min-h-0 flex-1 flex-col">
        {/* uix-spec §3 会话头：56px 折中高度 + 0.5px hairline；wsUrl 降为右侧弱标注 */}
        <div className="flex h-14 shrink-0 items-center gap-2 border-b-[0.5px] border-border px-5 text-sm font-medium">
          <span className="inline-flex items-center gap-2">
            <Bot aria-hidden className="size-4" />
            {title}
          </span>
          <span className="text-muted-foreground/70 ml-auto min-w-0 truncate text-xs font-normal">
            {endpoint.wsUrl}
          </span>
        </div>
        <PendingPermissionBanner endpointId={endpointId} />
        {activeSessionId === null ? (
          // uix-spec §3 空态 hero：居中「徽标 + 标题 + 说明 + 动作」栈替代裸文案
          <div className="flex flex-1 flex-col items-center justify-center gap-3 px-6 text-center">
            <span className="flex size-12 items-center justify-center rounded-2xl border-[0.5px] border-border bg-card shadow-sm">
              <Bot aria-hidden className="text-muted-foreground size-6" />
            </span>
            <p className="text-[26px] leading-tight font-medium">{t("acp.sessions.empty")}</p>
            <p className="text-muted-foreground text-[13px]">{t("acp.sessions.emptyHint")}</p>
            <AsyncButton
              type="button"
              size="sm"
              className="mt-1 rounded-full px-4"
              disabled={newSessionPending}
              action={newSessionAction}
              onSuccess={() => toastSuccess(t("chat.feedback.sessionCreated"))}
              loadingLabel={t("chat.feedback.sessionCreating")}
              resultHoldMs={300}
              data-testid="agent-new-session"
            >
              {t("chat.agentPane.newSession")}
            </AsyncButton>
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
  // 本机 agent + console 非 connected：引导卡接管（不提供注定失败的手动连接按钮）
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
        <ConnectFailureNotice
          lastError={lastError}
          closeInfo={closeInfo}
          wsUrl={endpoint.wsUrl}
          endpointId={endpointId}
        />
      ) : null}
    </div>
  );
}