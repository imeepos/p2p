// 工具时间线子组件（自 transcript.tsx 拆分，守 300 行红线）。
// AG-UI TOOL_CALL_* ↔ ACP tool_call(_update)，同 id 原地迁移见 transcript-model.ts。
// 四态呈现约定：pending 静默灰点 / in_progress 警示脉冲（进行时视觉）/
// completed 成功 / failed 红系整行高亮。
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { I18nKey } from "@/i18n/types";
import { StatusBadge, type StatusTone } from "@/views/shared/status-badge";
import type { ToolCallStatus } from "@/acp/protocol";
import { toolIoView, type Turn } from "@/acp/transcript-model";

export type ToolTurnModel = Extract<Turn, { kind: "tool" }>;

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

/** 工具入参/结果块：超过约 6 行默认折叠，展开开关带 aria-expanded/aria-controls */
function ToolIoBlock(props: { text: string; testId: string; muted?: boolean }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const view = toolIoView(props.text);
  return (
    <div className="flex flex-col items-start gap-0.5">
      <pre
        id={props.testId + "-body"}
        className={cn(
          "max-w-[90%] overflow-x-auto rounded px-2 py-1 text-xs whitespace-pre-wrap break-all",
          props.muted && "bg-muted/50 text-muted-foreground",
        )}
        data-testid={props.testId}
      >
        {view.collapsible && !open ? view.preview : props.text}
      </pre>
      {view.collapsible ? (
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="text-muted-foreground h-6 px-2 text-xs"
          aria-expanded={open}
          aria-controls={props.testId + "-body"}
          data-testid={props.testId + "-toggle"}
          onClick={() => setOpen((v) => !v)}
        >
          {open ? t("acp.tools.collapse") : t("acp.tools.expand")}
        </Button>
      ) : null}
    </div>
  );
}

/** 结果块：结算后（completed/failed）整块可折叠、默认展开；运行态仅按长度折叠 */
function ToolResultSection({ turn }: { turn: ToolTurnModel }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(true);
  const testId = "acp-tool-result-" + turn.toolCallId;
  return (
    <div className="flex flex-col items-start gap-0.5">
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className="text-muted-foreground h-6 px-2 text-xs"
        aria-expanded={open}
        aria-controls={testId + "-body"}
        data-testid={testId + "-toggle"}
        onClick={() => setOpen((v) => !v)}
      >
        {open ? t("acp.tools.collapse") : t("acp.tools.expand")}
      </Button>
      <div id={testId + "-body"} data-testid={testId}>
        {open ? (
          <ToolIoBlock
            text={turn.outputText}
            testId={"acp-tool-output-" + turn.toolCallId}
          />
        ) : null}
      </div>
    </div>
  );
}

/** 工具时间线节点：名称/四态徽章/入参/结果（设计 §8 工具行）；失败态红系整行高亮 */
export function ToolTurn({ turn }: { turn: ToolTurnModel }) {
  const { t } = useTranslation();
  const settled = turn.status === "completed" || turn.status === "failed";
  return (
    <div
      className={cn("ml-2 flex flex-col gap-1 border-l pl-3",
        turn.status === "failed"
          ? "border-l-destructive bg-destructive/5 rounded-r-md py-1"
          : "border-l-border/60",
      )}
      data-status={turn.status}
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
          data-testid={"acp-tool-dot-" + turn.toolCallId}
        />
        <span className="text-sm font-medium">{turn.title}</span>
        {turn.toolKind ? (
          <span className="bg-muted rounded px-1.5 py-0.5 text-xs">
            {t(("acp.tools.kind." + turn.toolKind) as I18nKey, { defaultValue: turn.toolKind })}
          </span>
        ) : null}
        <span data-testid={"acp-tool-status-" + turn.toolCallId}>
          <StatusBadge tone={TOOL_STATUS_TONE[turn.status]}>
            {t(TOOL_STATUS_KEY[turn.status])}
          </StatusBadge>
        </span>
      </div>
      {turn.inputText ? (
        <ToolIoBlock
          muted
          text={turn.inputText}
          testId={"acp-tool-input-" + turn.toolCallId} />
      ) : null}
      {turn.outputText ? (
        settled ? (
          <ToolResultSection turn={turn} />
        ) : (
          <ToolIoBlock
            text={turn.outputText}
            testId={"acp-tool-output-" + turn.toolCallId} />
        )
      ) : null}
    </div>
  );
}
