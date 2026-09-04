import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useAcpStore } from "@/acp/acp-store";
import type { Turn } from "@/acp/transcript-model";

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
            {t("acp.transcript.stopReason", { reason: turn.stopReason })}
          </span>
        ) : null}
      </div>
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
