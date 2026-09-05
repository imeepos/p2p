import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { Bot, Loader2, X } from "lucide-react";


import { PageHeader } from "@/components/page/page-header";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { PermissionNoticeBridge } from "@/acp/components/permission-notice-bridge";
import { useAcpStore } from "@/acp/acp-store";
import { CapabilitiesCard } from "@/acp/components/capabilities-card";
import { ConfigPanel } from "@/acp/components/config-panel";
import { ConnectionCard } from "@/acp/components/connection-card";
import { ConnectionDirectory } from "@/acp/components/connection-directory";
import { PermissionPanel } from "@/acp/components/permission-panel";
import { PromptComposer } from "@/acp/components/prompt-composer";
import { SessionSidebar } from "@/acp/components/session-sidebar";
import { Transcript } from "@/acp/components/transcript";
import { UsageBar } from "@/acp/components/usage-bar";
import { EmptyState } from "@/views/shared/empty-state";
import { Button } from "@/components/ui/button";

function ReconnectBanner() {
  const { t } = useTranslation();
  const reconnect = useAcpStore((s) => s.reconnect);
  const retryNow = useAcpStore((s) => s.retryNow);
  if (!reconnect) return null;
  return (
    <div
      className="border-warning/40 bg-warning/10 flex items-center justify-between gap-2 rounded-md border px-3 py-2 text-sm"
      data-testid="acp-reconnect-banner"
    >
      <span>{t("acp.connection.reconnectNotice", { attempt: reconnect.attempt, max: reconnect.max })}</span>
      <Button variant="outline" size="sm" onClick={retryNow} data-testid="acp-reconnect-now">
        {t("acp.reconnect.retryNow")}
      </Button>
    </div>
  );
}

/** 原会话失效引导：续连窗口过期走 fresh 重连后出现；指引用户走侧栏既有 resume 流程 */
function SessionLostNotice() {
  const { t } = useTranslation();
  const show = useAcpStore((s) => s.sessionLostNotice);
  const dismiss = useAcpStore((s) => s.dismissSessionLostNotice);
  if (!show) return null;
  return (
    <div
      className="border-warning/40 bg-warning/10 flex items-center justify-between gap-2 rounded-md border px-3 py-2 text-sm"
      data-testid="acp-session-lost-notice"
    >
      <span>{t("acp.reattach.sessionLost")}</span>
      <button
        type="button"
        aria-label={t("acp.reattach.dismissLost")}
        onClick={dismiss}
        data-testid="acp-session-lost-dismiss"
        className="text-muted-foreground hover:text-foreground"
      >
        <X className="size-4" aria-hidden />
      </button>
    </div>
  );
}

const REATTACH_AUTO_DISMISS_MS = 8_000;

/** 续连横幅：dsh/bridge/reattach 通知折射（apps/acp-agent/README.md 桥约定）。
 *  约 8 秒自动消失，也可手动关闭；补放 0 条（仅续连成功）与 N 条文案区分。 */
function ReattachBanner() {
  const { t } = useTranslation();
  const reattach = useAcpStore((s) => s.reattachNotice);
  const dismiss = useAcpStore((s) => s.dismissReattachNotice);
  useEffect(() => {
    if (!reattach) return;
    const timer = setTimeout(dismiss, REATTACH_AUTO_DISMISS_MS);
    return () => clearTimeout(timer);
  }, [reattach, dismiss]);
  if (!reattach) return null;
  const bannerKey = reattach.replayed > 0 ? "acp.reattach.banner" : "acp.reattach.bannerNone";
  return (
    <div
      className="border-success/40 bg-success/10 flex items-center justify-between gap-2 rounded-md border px-3 py-2 text-sm"
      data-testid="acp-reattach-banner"
    >
      <span>{t(bannerKey, { count: reattach.replayed })}</span>
      <button
        type="button"
        aria-label={t("acp.reattach.dismiss")}
        onClick={dismiss}
        data-testid="acp-reattach-dismiss"
        className="text-muted-foreground hover:text-foreground"
      >
        <X className="size-4" aria-hidden />
      </button>
    </div>
  );
}

/** 在线态连接状态条：全页唯一的常驻断开入口（连接卡在 online 态已卸载） */
function OnlineStatusBar() {
  const { t } = useTranslation();
  const activePeer = useAcpStore((s) => s.activePeer);
  const wsUrl = useAcpStore((s) => s.draft.wsUrl);
  const disconnect = useAcpStore((s) => s.disconnect);
  return (
    <div
      className="border-success/40 bg-success/10 flex items-center justify-between gap-2 rounded-md border px-3 py-2 text-sm"
      data-testid="acp-online-bar"
    >
      <span className="text-success shrink-0">{t("acp.feedback.connectedTo", { peer: activePeer ?? "" })}</span>
      <span className="text-muted-foreground min-w-0 truncate text-xs">{wsUrl}</span>
      <Button variant="outline" size="sm" onClick={disconnect} data-testid="acp-online-disconnect">
        {t("acp.connection.disconnect")}
      </Button>
    </div>
  );
}

/** 连接中加载态：主列不得无反馈空白（spinner + 握手文案） */
function ConnectingIndicator() {
  const { t } = useTranslation();
  return (
    <div
      className="text-muted-foreground flex items-center gap-2 py-8 text-sm"
      data-testid="acp-connecting-indicator"
    >
      <Loader2 className="size-4 animate-spin" aria-hidden />
      <span>{t("acp.feedback.connecting")}</span>
    </div>
  );
}

function MainColumn() {
  const { t } = useTranslation();
  const phase = useAcpStore((s) => s.phase);
  const activeSessionId = useAcpStore((s) => s.activeSessionId);
  return (
    <div className="col-span-12 flex min-h-0 flex-col gap-4 lg:col-span-9">
      {phase === "online" ? <OnlineStatusBar /> : null}
      <ReconnectBanner />
      <ReattachBanner />
      <SessionLostNotice />
      {phase === "connecting" ? (
        <ConnectingIndicator />
      ) : activeSessionId === null ? (
        <div data-testid="acp-main-empty">
          <EmptyState icon={Bot} title={t("acp.sessions.empty")} description={t("acp.sessions.emptyHint")} />
        </div>
      ) : (
        <>
          <ConfigPanel />
          <UsageBar />
          <Transcript sessionId={activeSessionId} />
          <PermissionPanel />
          <PromptComposer />
        </>
      )}
    </div>
  );
}

/** ACP 控制台页：连接管理 + 连接目录 + 会话侧栏 + transcript（设计 §8）。
 *  本页自持一份 ConfirmProvider：与 main.tsx 全站 Provider 嵌套无害，
 *  保证 AcpView 独立渲染（测试/嵌入场景）时确认纪律不缺位。 */
export function AcpView() {
  const phase = useAcpStore((s) => s.phase);
  return (
    <ConfirmProvider>
      <PermissionNoticeBridge />
      <div className="col-span-12 grid grid-cols-12 gap-4">
        <PageHeader titleKey="acp.title" descriptionKey="acp.description" />
      {phase === "online" ? (
        <CapabilitiesCard />
      ) : (
        <div className="col-span-12 grid gap-4 lg:grid-cols-2">
          <ConnectionCard />
          <ConnectionDirectory />
        </div>
      )}
      <div className="col-span-12 grid grid-cols-12 gap-4">
        <div className="col-span-12 lg:col-span-3">
          <SessionSidebar />
        </div>
        <MainColumn />
      </div>
      </div>
    </ConfirmProvider>
  );
}