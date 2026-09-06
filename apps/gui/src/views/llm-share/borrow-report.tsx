import { useTranslation } from "react-i18next";

import type { I18nKey } from "@/i18n/types";

import { StatusBadge, type StatusTone } from "@/views/shared/status-badge";

import type { LlmBorrowReport } from "./types";

// 借用结果三态渲染（§16.2-1/2）：done 正常收据；stream_broken 渲染
// 「估算账单·72h 争议窗」中性态禁渲染失败；rejected 展示拒绝码原样透出。
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

export function BorrowReportCard({ report }: { report: LlmBorrowReport }) {
  const { t } = useTranslation();
  const tone = toneOf(report.status);
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
      {report.code ? (
        <p className="mt-1 text-xs">
          {t("llmShare.borrow.rejectedCodeLabel")}:{" "}
          <code data-testid="reject-code" className="font-mono">
            {report.code}
          </code>
        </p>
      ) : null}
      <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-xs">
        <dt className="text-muted-foreground">{t("llmShare.borrow.reqIdLabel")}</dt>
        <dd className="truncate font-mono" title={report.receipt.reqId}>
          {report.receipt.reqId}
        </dd>
        <dt className="text-muted-foreground">{t("llmShare.borrow.appendedLabel")}</dt>
        <dd>{String(report.receipt.appended)}</dd>
        <dt className="text-muted-foreground">{t("llmShare.borrow.sseCountLabel")}</dt>
        <dd>{report.sseCount}</dd>
        {report.usage ? (
          <>
            <dt className="text-muted-foreground">{t("llmShare.borrow.usageLabel")}</dt>
            <dd>
              {report.usage.input}/{report.usage.output}
            </dd>
          </>
        ) : null}
        <dt className="text-muted-foreground">disputeWindowSecs</dt>
        <dd>{report.receipt.disputeWindowSecs}</dd>
      </dl>
      {report.message ? (
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
