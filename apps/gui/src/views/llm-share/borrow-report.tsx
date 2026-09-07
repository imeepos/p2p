import { useTranslation } from "react-i18next";

import type { I18nKey } from "@/i18n/types";

import { CopyButton } from "@/components/monitor/copy-button";
import { StatusBadge, type StatusTone } from "@/views/shared/status-badge";

import type { LlmBorrowReport, LlmRejectCode } from "./types";

// 借用结果三态渲染（§16.2-1/2）：done 正常收据；stream_broken 渲染
// 「估算账单·争议窗」中性态禁渲染失败；rejected 展示人话原因，拒绝码
// 原样保留并入可复制详情（R2-07，F06 先例）。
function toneOf(status: LlmBorrowReport["status"]): StatusTone {
  if (status === "done") return "success";
  if (status === "rejected") return "warning";
  return "neutral";
}

const BADGE_KEY: Record<LlmBorrowReport["status"], I18nKey> = {
  done: "llmShare.borrow.statusDone",
  stream_broken: "llmShare.borrow.streamBrokenBadge",
  rejected: "llmShare.borrow.statusRejected",
};

const REJECT_REASON_KEY: Record<LlmRejectCode, I18nKey> = {
  not_allowlisted: "llmShare.borrow.rejectNotAllowlisted",
  model_not_served: "llmShare.borrow.rejectModelNotServed",
  freeze_insufficient: "llmShare.borrow.rejectFreezeInsufficient",
  concurrency_exceeded: "llmShare.borrow.rejectConcurrencyExceeded",
};

function InfoRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <>
      <dt className="text-muted-foreground">{label}</dt>
      <dd className={mono ? "truncate font-mono" : undefined}>{value}</dd>
    </>
  );
}

function RejectedBlock({ report }: { report: LlmBorrowReport }) {
  const { t } = useTranslation();
  const reason = report.code ? t(REJECT_REASON_KEY[report.code]) : "";
  return (
    <div className="mt-1 flex flex-col gap-1" data-testid="reject-reason">
      {reason ? <p className="text-xs">{t("llmShare.borrow.rejectedReasonLabel")}: {reason}</p> : null}
      {report.code ? (
        <p className="flex items-center gap-1 text-xs">
          {t("llmShare.borrow.rejectedCodeLabel")}:{" "}
          <code data-testid="reject-code" className="font-mono">
            {report.code}
          </code>
          <CopyButton
            value={`${reason}\ncode: ${report.code}`}
            className="size-6"
            aria-label={t("common.feedback.copyDetail")}
          />
        </p>
      ) : null}
    </div>
  );
}

export function BorrowReportCard({ report }: { report: LlmBorrowReport }) {
  const { t } = useTranslation();
  const tone = toneOf(report.status);
  const disputeHours = Math.round(report.receipt.disputeWindowSecs / 3600);
  return (
    <div
      data-testid="borrow-report"
      data-tone={tone}
      className="rounded-md border p-3 text-sm"
    >
      <div className="flex items-center gap-2">
        <StatusBadge tone={tone} dot>
          {t(BADGE_KEY[report.status])}
        </StatusBadge>
        {report.status === "stream_broken" ? (
          <span className="text-muted-foreground text-xs">
            {t("llmShare.borrow.statusStreamBroken")}
          </span>
        ) : null}
      </div>
      {report.status === "stream_broken" ? (
        <p className="text-muted-foreground mt-1 text-xs">
          {t("llmShare.borrow.streamBrokenHint")}
        </p>
      ) : null}
      {report.status === "rejected" ? <RejectedBlock report={report} /> : null}
      <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-xs">
        <InfoRow label={t("llmShare.borrow.reqIdLabel")} value={report.receipt.reqId} mono />
        <InfoRow
          label={t("llmShare.borrow.appendedLabel")}
          value={t(report.receipt.appended ? "llmShare.borrow.appendedYes" : "llmShare.borrow.appendedNo")}
        />
        <InfoRow label={t("llmShare.borrow.sseCountLabel")} value={String(report.sseCount)} />
        {report.usage ? (
          <InfoRow
            label={t("llmShare.borrow.usageLabel")}
            value={`${report.usage.input}/${report.usage.output}`}
          />
        ) : null}
        {disputeHours > 0 ? (
          <InfoRow
            label={t("llmShare.borrow.disputeWindowLabel")}
            value={t("llmShare.borrow.disputeWindowValue", { hours: disputeHours })}
          />
        ) : null}
      </dl>
      {report.message && report.status !== "rejected" ? (
        <p className="mt-2">
          <span className="text-muted-foreground text-xs">
            {t("llmShare.borrow.ssePreviewLabel")}
          </span>
          <span className="mt-0.5 block max-h-16 overflow-hidden font-mono text-xs break-all">
            {report.message}
          </span>
        </p>
      ) : null}
    </div>
  );
}
