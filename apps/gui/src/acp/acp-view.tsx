import { useTranslation } from "react-i18next";
import { Bot } from "lucide-react";

import { PageHeader } from "@/components/page/page-header";
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

/** 续连横幅：dsh/bridge/reattach 通知折射（apps/acp-agent/README.md 桥约定） */
function ReattachBanner() {
  const { t } = useTranslation();
  const reattach = useAcpStore((s) => s.reattachNotice);
  if (!reattach) return null;
  return (
    <div
      className="border-success/40 bg-success/10 rounded-md border px-3 py-2 text-sm"
      data-testid="acp-reattach-banner"
    >
      {t("acp.reattach.banner", { count: reattach.replayed })}
    </div>
  );
}

function MainColumn() {
  const { t } = useTranslation();
  const activeSessionId = useAcpStore((s) => s.activeSessionId);
  return (
    <div className="col-span-12 flex min-h-0 flex-col gap-4 lg:col-span-9">
      <ReconnectBanner />
      <ReattachBanner />
      {activeSessionId === null ? (
        <EmptyState icon={Bot} title={t("acp.sessions.empty")} description={t("acp.sessions.emptyHint")} />
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

/** ACP 控制台页：连接管理 + 连接目录 + 会话侧栏 + transcript（设计 §8） */
export function AcpView() {
  const phase = useAcpStore((s) => s.phase);
  return (
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
  );
}