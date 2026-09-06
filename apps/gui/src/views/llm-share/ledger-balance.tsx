import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Scale } from "lucide-react";

import type { I18nKey } from "@/i18n/types";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { errorText } from "@/views/shared/form-flow";
import { EmptyState } from "@/views/shared/empty-state";
import { StatusBadge, type StatusTone } from "@/views/shared/status-badge";

import type { LlmBalanceDirection, LlmBalanceGroup, LlmShareBackend } from "./types";

// 双边账本·净差视图（§16.1 balance）：按 lender+period 分组，
// 正负号 = 借贷方向（本机为 lender 记正，为 borrower 记负）。
const DIRECTION_TONE: Record<LlmBalanceDirection, StatusTone> = {
  lent: "success",
  borrowed: "warning",
  flat: "neutral",
};

const DIRECTION_KEY: Record<LlmBalanceDirection, I18nKey> = {
  lent: "llmShare.ledger.directionLent",
  borrowed: "llmShare.ledger.directionBorrowed",
  flat: "llmShare.ledger.directionFlat",
};

function netText(net: number): string {
  return net > 0 ? `+${net}` : String(net);
}

function BalanceRow({ row }: { row: LlmBalanceGroup }) {
  const { t } = useTranslation();
  return (
    <div
      data-testid="balance-row"
      data-direction={row.direction}
      className="flex items-center gap-3 rounded-md border p-2 text-xs"
    >
      <StatusBadge tone={DIRECTION_TONE[row.direction]} dot>
        {t(DIRECTION_KEY[row.direction])}
      </StatusBadge>
      <span className="min-w-0 flex-1 truncate font-mono" title={row.lender}>
        {row.lender}
      </span>
      <span className="text-muted-foreground">{row.period}</span>
      <span className="font-mono" data-testid="balance-net">
        {netText(row.netAmount)}
      </span>
      <span className="text-muted-foreground">
        ({row.lentOut}/-{row.borrowed}, {row.entries})
      </span>
    </div>
  );
}

export function LedgerBalanceCard({ backend }: { backend: LlmShareBackend }) {
  const { t } = useTranslation();
  const [rows, setRows] = useState<LlmBalanceGroup[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const groups = await backend.ledgerBalance();
        if (!cancelled) {
          setRows(groups);
          setError(null);
        }
      } catch (e) {
        console.warn("[llm-share] balance 读取失败", e);
        if (!cancelled) setError(errorText(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [backend]);

  return (
    <Card data-testid="ledger-balance">
      <CardHeader>
        <CardTitle className="text-sm">{t("llmShare.ledger.balanceTitle")}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {error ? <p role="alert" className="text-destructive text-xs">{error}</p> : null}
        {rows === null ? null : rows.length === 0 ? (
          <EmptyState icon={Scale} title={t("llmShare.ledger.emptyBalance")} />
        ) : (
          rows.map((row) => <BalanceRow key={`${row.lender}#${row.period}`} row={row} />)
        )}
      </CardContent>
    </Card>
  );
}
