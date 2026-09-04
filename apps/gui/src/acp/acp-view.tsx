import { useTranslation } from "react-i18next";

import { PageHeader } from "@/components/page/page-header";
import { useAcpStore } from "@/acp/acp-store";
import { CapabilitiesCard } from "@/acp/components/capabilities-card";
import { ConnectionCard } from "@/acp/components/connection-card";
import { PromptComposer } from "@/acp/components/prompt-composer";
import { SessionSidebar } from "@/acp/components/session-sidebar";
import { Transcript } from "@/acp/components/transcript";
import { EmptyState } from "@/views/shared/empty-state";
import { Bot } from "lucide-react";

function ReconnectBanner() {
  const { t } = useTranslation();
  const reconnect = useAcpStore((s) => s.reconnect);
  if (!reconnect) return null;
  return (
    <div
      className="border-warning/40 bg-warning/10 rounded-md border px-3 py-2 text-sm"
      data-testid="acp-reconnect-banner"
    >
      {t("acp.connection.reconnectNotice", { attempt: reconnect.attempt, max: reconnect.max })}
    </div>
  );
}

function MainColumn() {
  const { t } = useTranslation();
  const activeSessionId = useAcpStore((s) => s.activeSessionId);
  return (
    <div className="col-span-12 flex min-h-0 flex-col gap-4 lg:col-span-9">
      <ReconnectBanner />
      {activeSessionId === null ? (
        <EmptyState icon={Bot} title={t("acp.sessions.empty")} description={t("acp.sessions.emptyHint")} />
      ) : (
        <>
          <Transcript sessionId={activeSessionId} />
          <PromptComposer />
        </>
      )}
    </div>
  );
}

/** ACP 控制台页：连接管理 + 会话侧栏 + 流式 transcript（设计 §8 前四行） */
export function AcpView() {
  const phase = useAcpStore((s) => s.phase);
  return (
    <div className="col-span-12 grid grid-cols-12 gap-4">
      <PageHeader titleKey="acp.title" descriptionKey="acp.description" />
      {phase === "online" ? <CapabilitiesCard /> : <ConnectionCard />}
      <div className="col-span-12 grid grid-cols-12 gap-4">
        <div className="col-span-12 lg:col-span-3">
          <SessionSidebar />
        </div>
        <MainColumn />
      </div>
    </div>
  );
}
