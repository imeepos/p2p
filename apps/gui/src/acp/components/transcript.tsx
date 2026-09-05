import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { I18nKey } from "@/i18n/types";
import { StatusBadge, type StatusTone } from "@/views/shared/status-badge";
import { useAcpStore } from "@/acp/acp-store";
import type { ToolCallStatus } from "@/acp/protocol";
import type { Turn } from "@/acp/transcript-model";

/** ACP v1 stopReason 各态文案；未知值回退原样透传 */
const STOP_KEY: Record<string, I18nKey> = {
  end_turn: "acp.transcript.stop.end_turn",
  max_tokens: "acp.transcript.stop.max_tokens",
  max_turn_requests: "acp.transcript.stop.max_turn_requests",
  refusal: "acp.transcript.stop.refusal",
  cancelled: "acp.transcript.stop.cancelled",
};

const TOOL_STATUS_TONE: Record<ToolCallStatus, StatusTone> = {
  pending: "neutral",
  in_progress: "warning",
  completed: "success",
  failed: "danger",
};

const TOOL_STATUS_KEY: Record<ToolCallStatus, I18nKey> = {
  pending: "acp.tools.status.pending",
  in_progress: "acp.tools.status.in_progress",
  completed: "acp.tools.status.completed",
  failed: "acp.tools.status.failed",
};

function stopReasonText(t: (key: I18nKey, opts?: Record<string, unknown>) => string, reason: string): string {
  const key = STOP_KEY[reason];
  return key ? t(key) : t("acp.transcript.stopReason", { reason });
}

interface TranscriptProps {
  sessionId: string;
}

/** 思考面板：agent_thought_chunk 归并为可折叠块，默认收起 */
function ThoughtTurn({ sessionId, turn }: { sessionId: string; turn: Extract<Turn, { kind: "thought" }> }) {
  const { t } = useTranslation();
  const toggle = useAcpStore((s) => s.toggleThought);
  return (
    <div className="w-full" data-testid={"acp-turn-thought-" + turn.id}>
      <Button
        variant="ghost"
        size="sm"
        className="text-muted-foreground h-7 px-2"
        aria-expanded={turn.open}
        data-testid={"acp-thought-toggle-" + turn.id}
        onClick={() => toggle(sessionId, turn.id)}
      >
        {turn.open ? t("acp.transcript.thoughtHide") : t("acp.transcript.thoughtShow")}
      </Button>
      {turn.open ? (
        <p className="text-muted-foreground mx-2 mb-2 whitespace-pre-wrap text-xs leading-relaxed">
          {turn.text}
        </p>
      ) : null}
    </div>
  );
}

function AssistantTurn({ turn }: { turn: Extract<Turn, { kind: "assistant" }> }) {
  const { t } = useTranslation();
  return (
    <div className="flex justify-start" data-testid={"acp-turn-assistant-" + turn.id}>
      <div
        className={cn(
          "bg-muted text-foreground max-w-[80%] rounded-2xl rounded-bl-sm px-3 py-2 text-sm",
          turn.streaming && "animate-pulse",
        )}
      >
        <p className="whitespace-pre-wrap break-words">{turn.text}</p>
        {turn.streaming ? (
          <span className="text-muted-foreground text-xs" data-testid="acp-streaming-badge">
            {t("acp.transcript.streaming")}
          </span>
        ) : turn.stopReason ? (
          <span className="text-muted-foreground text-xs" data-testid={"acp-stop-reason-" + turn.id}>
            {stopReasonText(t, turn.stopReason)}
          </span>
        ) : null}
      </div>
    </div>
  );
}


/** 工具时间线节点：名称/状态徽章/入参/结果（设计 §8 工具行） */
function ToolTurn({ turn }: { turn: Extract<Turn, { kind: "tool" }> }) {
  const { t } = useTranslation();
  return (
    <div
      className="border-l-border/60 ml-2 flex flex-col gap-1 border-l pl-3"
      data-testid={"acp-turn-tool-" + turn.toolCallId}
    >
      <div className="flex flex-wrap items-center gap-2">
        <span
          className={cn("size-2 shrink-0 rounded-full",
            turn.status === "failed" && "bg-destructive",
            turn.status === "completed" && "bg-success",
            turn.status === "in_progress" && "bg-warning animate-pulse",
            turn.status === "pending" && "bg-muted-foreground/40",
          )}
        />
        <span className="text-sm font-medium">{turn.title}</span>
        {turn.toolKind ? (
          <span className="bg-muted rounded px-1.5 py-0.5 text-xs">
            {t(("acp.tools.kind." + turn.toolKind) as I18nKey, { defaultValue: turn.toolKind })}
          </span>
        ) : null}
        <span data-testid={"acp-tool-status-" + turn.toolCallId}>
          <StatusBadge tone={TOOL_STATUS_TONE[turn.status]}>{t(TOOL_STATUS_KEY[turn.status])}</StatusBadge>
        </span>
      </div>
      {turn.inputText ? (
        <pre className="bg-muted/50 max-w-[90%] overflow-x-auto rounded px-2 py-1 text-xs whitespace-pre-wrap break-all text-muted-foreground"
          data-testid={"acp-tool-input-" + turn.toolCallId}>
          {turn.inputText}
        </pre>
      ) : null}
      {turn.outputText ? (
        <pre className="max-w-[90%] overflow-x-auto rounded px-2 py-1 text-xs whitespace-pre-wrap break-all"
          data-testid={"acp-tool-output-" + turn.toolCallId}>
          {turn.outputText}
        </pre>
      ) : null}
    </div>
  );
}

function UserTurn({ turn }: { turn: Extract<Turn, { kind: "user" }> }) {
  const { t } = useTranslation();
  return (
    <div className="flex justify-end" data-testid={"acp-turn-user-" + turn.id}>
      <div className="bg-primary text-primary-foreground max-w-[80%] rounded-2xl rounded-br-sm px-3 py-2 text-sm">
        <p className="whitespace-pre-wrap break-words">{turn.text}</p>
        <span className="text-primary-foreground/70 text-right text-xs">{t("acp.transcript.user")}</span>
      </div>
    </div>
  );
}

export function Transcript({ sessionId }: TranscriptProps) {
  const { t } = useTranslation();
  const transcript = useAcpStore((s) => s.transcripts[sessionId]);
  const turns = transcript?.turns ?? [];
  if (turns.length === 0) {
    return (
      <div className="text-muted-foreground flex flex-1 items-center justify-center text-sm">
        {t("acp.transcript.empty")}
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-2" data-testid="acp-transcript">
      {turns.map((turn) => {
        if (turn.kind === "thought") return <ThoughtTurn key={turn.id} sessionId={sessionId} turn={turn} />;
        if (turn.kind === "assistant") return <AssistantTurn key={turn.id} turn={turn} />;
        if (turn.kind === "tool") return <ToolTurn key={turn.id} turn={turn} />;
        return <UserTurn key={turn.id} turn={turn} />;
      })}
      {transcript && transcript.ignoredUpdates > 0 ? (
        <p className="text-muted-foreground text-xs">
          {t("acp.transcript.ignored", { count: transcript.ignoredUpdates })}
        </p>
      ) : null}
    </div>
  );
}
