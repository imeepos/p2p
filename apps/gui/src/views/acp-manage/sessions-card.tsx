// 会话管理卡：本机 agent 的会话清单（复用 acp-store 既有连接态），
// 未连接时给出去处（消息中心 agent 会话），不在这里复制一套连接逻辑。
import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";
import { MessagesSquare } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useAcpStore } from "@/acp/acp-store";
import { EmptyState } from "@/views/shared/empty-state";

export function SessionsCard() {
  const { t } = useTranslation();
  const phase = useAcpStore((s) => s.phase);
  const activePeer = useAcpStore((s) => s.activePeer);
  const sessions = useAcpStore((s) => s.sessions);
  const activeSessionId = useAcpStore((s) => s.activeSessionId);
  const online = phase === "online";

  return (
    <Card data-testid="acp-manage-sessions-card">
      <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-base">{t("acpManage.sessions.title")}</CardTitle>
        {online ? (
          <span className="text-muted-foreground text-xs" data-testid="acp-sessions-count">
            {t("acpManage.sessions.count", { count: sessions.length })}
          </span>
        ) : null}
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {!online ? (
          <div className="flex flex-col gap-2" data-testid="acp-sessions-offline">
            <EmptyState icon={MessagesSquare} title={t("acpManage.sessions.notConnected")} description={t("acpManage.sessions.offlineHint")} />
            <Button asChild size="sm" variant="outline" className="self-start">
              <Link to="/chat?kind=agent" data-testid="acp-sessions-go-chat">{t("acpManage.sessions.goChat")}</Link>
            </Button>
          </div>
        ) : sessions.length === 0 ? (
          <p className="text-muted-foreground text-sm" data-testid="acp-sessions-empty">
            {t("acpManage.sessions.empty")}
          </p>
        ) : (
          sessions.map((session) => (
            <div
              key={session.sessionId}
              className="flex items-center justify-between gap-2 rounded-md border px-2 py-1.5"
              data-testid={"acp-sessions-row-" + session.sessionId}
            >
              <div className="min-w-0">
                <p className="truncate text-sm font-medium">{session.title || session.sessionId}</p>
                {session.cwd ? (
                  <p className="text-muted-foreground truncate font-mono text-xs" title={session.cwd}>{session.cwd}</p>
                ) : null}
              </div>
              {session.sessionId === activeSessionId ? (
                <span className="shrink-0 text-success text-xs">{t("acpManage.sessions.active")}</span>
              ) : null}
            </div>
          ))
        )}
        {online && activePeer ? (
          <p className="text-muted-foreground truncate text-xs" data-testid="acp-sessions-peer">
            {t("acpManage.sessions.connectedTo", { peer: activePeer })}
          </p>
        ) : null}
      </CardContent>
    </Card>
  );
}