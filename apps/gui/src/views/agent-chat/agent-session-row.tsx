import { useTranslation } from "react-i18next";
import type { SessionSummary } from "@/acp/protocol";
import { formatRelative } from "@/lib/relative-time";
import type { Locale } from "@/i18n";
import { cn } from "@/lib/utils";

// ACS1 会话行：标题 + 右侧相对时间（参照图密度）。相对时间只认真实交互时刻
// （见 sessionActivityMs 说明），拿不到就整格不显，不臆造时间。
export function AgentSessionRow(props: {
  session: SessionSummary;
  active: boolean;
  online: boolean;
  activityMs: number;
  onOpen: (sessionId: string) => void;
}) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  const title = props.session.title?.trim() || props.session.sessionId;
  return (
    <button
      type="button"
      disabled={!props.online}
      onClick={() => props.onOpen(props.session.sessionId)}
      aria-current={props.active ? "true" : undefined}
      aria-label={t("agentChat.sidebar.sessionsAria", { name: title })}
      title={props.session.sessionId}
      data-testid={"agent-session-row-" + props.session.sessionId}
      className={cn(
        "flex w-full items-center gap-2 rounded-lg py-1.5 pr-2 pl-8 text-left transition-colors",
        props.active ? "bg-accent font-medium" : "hover:bg-wx-hover",
        !props.online && "opacity-60",
      )}
    >
      <span className="min-w-0 flex-1 truncate text-sm">{title}</span>
      {props.activityMs > 0 ? (
        <time
          dateTime={new Date(props.activityMs).toISOString()}
          className="text-muted-foreground shrink-0 text-[11px]"
          data-testid={"agent-session-time-" + props.session.sessionId}
        >
          {formatRelative(props.activityMs, locale)}
        </time>
      ) : null}
    </button>
  );
}
