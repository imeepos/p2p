import { MessagesSquare, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { cn } from "@/lib/utils";
import { useAcpStore } from "@/acp/acp-store";
import { EmptyState } from "@/views/shared/empty-state";

function SessionRow(props: {
  sessionId: string;
  title: string;
  active: boolean;
  onResume: (sessionId: string) => void;
  onClose: (sessionId: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <div
      className={cn(
        "flex items-center justify-between gap-2 rounded-md border px-2 py-1.5",
        props.active && "border-primary/40 bg-primary/5",
      )}
      data-testid={"acp-session-row-" + props.sessionId}
    >
      <button
        type="button"
        className="min-w-0 flex-1 text-left"
        onClick={() => props.onResume(props.sessionId)}
        title={t("acp.sessions.resume")}
        aria-current={props.active ? "true" : undefined}
      >
        <p className="truncate text-sm font-medium">{props.title}</p>
        <p className="text-muted-foreground truncate text-xs">{props.sessionId}</p>
      </button>
      <Button
        size="icon"
        variant="ghost"
        className="size-7"
        onClick={() => props.onClose(props.sessionId)}
        aria-label={t("acp.sessions.close")}
        data-testid={"acp-session-close-" + props.sessionId}
      >
        ×
      </Button>
    </div>
  );
}

export function SessionSidebar() {
  const { t } = useTranslation();
  const sessions = useAcpStore((s) => s.sessions);
  const activeSessionId = useAcpStore((s) => s.activeSessionId);
  const online = useAcpStore((s) => s.phase) === "online";
  const newSession = useAcpStore((s) => s.newSession);
  const resumeSession = useAcpStore((s) => s.resumeSession);
  const closeSession = useAcpStore((s) => s.closeSession);

  return (
    <Card className="flex flex-col">
      <CardHeader className="flex-row items-center justify-between space-y-0">
        <CardTitle className="text-base">{t("acp.sessions.card")}</CardTitle>
        <Button
          size="sm"
          variant="outline"
          disabled={!online}
          onClick={() => void newSession()}
          data-testid="acp-session-new"
        >
          <Plus className="size-4" aria-hidden />
          {t("acp.sessions.new")}
        </Button>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-2">
        {sessions.length === 0 ? (
          <EmptyState
            icon={MessagesSquare}
            title={t("acp.sessions.empty")}
            description={t("acp.sessions.emptyHint")}
          />
        ) : (
          sessions.map((session) => (
            <SessionRow
              key={session.sessionId}
              sessionId={session.sessionId}
              title={session.title ?? session.sessionId}
              active={session.sessionId === activeSessionId}
              onResume={(id) => void resumeSession(id)}
              onClose={(id) => void closeSession(id)}
            />
          ))
        )}
      </CardContent>
    </Card>
  );
}
