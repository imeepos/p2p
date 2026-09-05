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
import { useConfirm } from "@/components/feedback/confirm-provider";
import { EmptyState } from "@/views/shared/empty-state";

function SessionRow(props: {
  sessionId: string;
  title: string;
  active: boolean;
  online: boolean;
  onResume: (sessionId: string) => void;
  onClose: (sessionId: string) => void;
}) {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const handleClose = () => {
    // 破坏性操作确认纪律（P2）：关闭会话先过全站确认弹框，取消则不关
    void confirm({
      title: t("acp.sessions.closeConfirmTitle"),
      description: t("acp.sessions.closeConfirmDescription", { title: props.title }),
      confirmText: t("acp.sessions.closeConfirmAction"),
      cancelText: t("acp.cancel"),
      destructive: true,
    }).then((ok) => {
      if (ok) props.onClose(props.sessionId);
    });
  };
  return (
    <div
      className={cn(
        // gap-3 拉开 resume 与 close 的防误触间距（P2）
        "flex items-center justify-between gap-3 rounded-md border px-2 py-1.5",
        props.active && "border-primary/40 bg-primary/5",
      )}
      data-testid={"acp-session-row-" + props.sessionId}
    >
      <button
        type="button"
        className="min-w-0 flex-1 text-left"
        disabled={!props.online}
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
        className="size-7 shrink-0"
        disabled={!props.online}
        onClick={handleClose}
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
              online={online}
              onResume={(id) => void resumeSession(id)}
              onClose={(id) => void closeSession(id)}
            />
          ))
        )}
      </CardContent>
    </Card>
  );
}
