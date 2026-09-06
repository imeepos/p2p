import { useEffect, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { I18nKey } from "@/i18n/types";
import { StatusBadge } from "@/views/shared/status-badge";
import { useAcpStore } from "@/acp/acp-store";
import type { Turn } from "@/acp/transcript-model";
import { ToolTurn } from "./transcript-tools";

/** ACP v1 stopReason 各态文案；未知值回退原样透传 */
const STOP_KEY: Record<string, I18nKey> = {
  end_turn: "acp.transcript.stop.end_turn",
  max_tokens: "acp.transcript.stop.max_tokens",
  max_turn_requests: "acp.transcript.stop.max_turn_requests",
  refusal: "acp.transcript.stop.refusal",
  cancelled: "acp.transcript.stop.cancelled",
  error: "acp.transcript.stop.error",
};

/** prompt 错误结算态（stopReason=error）呈现为红色系失败徽章的判定 */
function isErrorStop(reason: string): boolean {
  return reason === "error";
}

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
        <p
          className="text-muted-foreground mx-2 mb-2 max-w-prose rounded-md bg-muted/40 px-3 py-2 whitespace-pre-wrap text-xs leading-6"
          data-testid={"acp-thought-body-" + turn.id}
        >
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
        ) : turn.stopReason && isErrorStop(turn.stopReason) ? (
          <span data-testid={"acp-stop-reason-" + turn.id}>
            <StatusBadge tone="danger" dot>{stopReasonText(t, turn.stopReason)}</StatusBadge>
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

/** AG-UI RUN_STARTED→RUN_FINISHED/ERROR 窗口的会话区进行中条：
 *  pending 派生自 store，结算即自动消失，无独立生命周期需要清理 */
function RunActiveStrip() {
  const { t } = useTranslation();
  return (
    <div
      className="text-muted-foreground flex items-center gap-2 text-xs"
      data-testid="acp-run-active"
    >
      <span className="bg-warning size-2 animate-pulse rounded-full" />
      {t("acp.transcript.runActive")}
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

/** 近底判定阈值：距底小于该值视为跟随（对齐 chat message-list 既有行为） */
const STICK_BOTTOM_PX = 64;

export function Transcript({ sessionId }: TranscriptProps) {
  const { t } = useTranslation();
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const stickBottomRef = useRef(true);
  const transcript = useAcpStore((s) => s.transcripts[sessionId]);
  // AG-UI RUN_STARTED→RUN_FINISHED/ERROR 窗口：回合进行中派生态
  const runActive = useAcpStore((s) => s.promptPendingBySession[sessionId] ?? false);
  // 兜底空数组须稳定引用，否则每次渲染都会重触发滚动 effect（exhaustive-deps）
  const turns = useMemo(() => transcript?.turns ?? [], [transcript]);

  useEffect(() => {
    // 切会话强制恢复跟随（对齐 chat：switching peer forces stick-to-bottom）
    stickBottomRef.current = true;
  }, [sessionId]);

  useEffect(() => {
    const el = scrollRef.current;
    if (el && stickBottomRef.current) el.scrollTop = el.scrollHeight;
  }, [turns, sessionId]);

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    stickBottomRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < STICK_BOTTOM_PX;
  };

  if (turns.length === 0) {
    return (
      <div className="text-muted-foreground flex flex-1 items-center justify-center text-sm">
        {t("acp.transcript.empty")}
      </div>
    );
  }
  return (
    <div
      ref={scrollRef}
      onScroll={onScroll}
      className="scroll-slim flex max-h-[65vh] min-h-0 flex-col gap-2 overflow-y-auto"
      data-testid="acp-transcript-scroll"
    >
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
        {runActive ? <RunActiveStrip /> : null}
      </div>
    </div>
  );
}