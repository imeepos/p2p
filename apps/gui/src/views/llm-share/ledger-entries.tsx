import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { EntityCombobox, type PickerOption } from "@/components/picker";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatDateTime } from "@/lib/format";
import type { Locale } from "@/i18n";
import { errorText } from "@/views/shared/form-flow";
import { shortPeerId } from "@/lib/peer-name";
import { PeerNameCell } from "@/views/shared/peer-name-cell";
import { StatusBadge } from "@/views/shared/status-badge";
import { usePeerPickerOptions } from "./peer-options";

import { subscribeLedgerMutated } from "./ledger-sync";
import type { LlmLedgerEntry, LlmLedgerFilter, LlmShareBackend } from "./types";

interface FilterValues {
  lender: string;
  borrower: string;
  period: string;
}

const EMPTY_FILTER: FilterValues = { lender: "", borrower: "", period: "" };

function filterOf(values: FilterValues): LlmLedgerFilter {
  const filter: LlmLedgerFilter = {};
  if (values.lender.trim()) filter.lender = values.lender.trim();
  if (values.borrower.trim()) filter.borrower = values.borrower.trim();
  if (values.period.trim()) filter.period = values.period.trim();
  return filter;
}

// 过滤候选：流水里出现过的 peer 与在册对端取并集（已知名优先，其余缩略）
function peerFilterOptions(
  peerOptions: PickerOption[],
  rows: LlmLedgerEntry[] | null,
): PickerOption[] {
  const known = new Map(peerOptions.map((o) => [o.value, o]));
  for (const row of rows ?? []) {
    for (const peer of [row.lender, row.borrower]) {
      if (!known.has(peer)) {
        known.set(peer, { value: peer, label: shortPeerId(peer), hint: shortPeerId(peer) });
      }
    }
  }
  return [...known.values()].sort((a, b) => a.label.localeCompare(b.label));
}

function EntryRow({ entry }: { entry: LlmLedgerEntry }) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language as Locale;
  return (
    <TableRow data-testid="ledger-row">
      <TableCell className="max-w-36 truncate font-mono text-xs" title={entry.reqId}>
        {entry.reqId}
      </TableCell>
      <TableCell className="text-xs">{entry.period}</TableCell>
      <TableCell className="max-w-32">
        <PeerNameCell peerId={entry.lender} />
      </TableCell>
      <TableCell className="max-w-32">
        <PeerNameCell peerId={entry.borrower} />
      </TableCell>
      <TableCell className="text-xs">{entry.model}</TableCell>
      <TableCell className="font-mono text-xs">
        {entry.tokens} ({entry.input}/{entry.output})
      </TableCell>
      <TableCell>
        {entry.estimated ? (
          <StatusBadge tone="neutral">{t("llmShare.ledger.estimatedFlag")}</StatusBadge>
        ) : (
          <span className="text-muted-foreground text-xs">-</span>
        )}
      </TableCell>
      <TableCell className="text-xs" data-testid="ledger-row-ts">
        {formatDateTime(entry.ts * 1000, locale)}
      </TableCell>
    </TableRow>
  );
}

export function LedgerEntriesCard({
  backend,
  onQueried,
}: {
  backend: LlmShareBackend;
  /** 每次查询完成后回调：流水卡「查询」联动净差卡重拉（R2-01） */
  onQueried?: () => void;
}) {
  const { t } = useTranslation();
  const peerOptions = usePeerPickerOptions();
  const [filters, setFilters] = useState<FilterValues>(EMPTY_FILTER);
  const [rows, setRows] = useState<LlmLedgerEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const filterOptions = peerFilterOptions(peerOptions, rows);

  const query = useCallback(
    async (values: FilterValues) => {
      try {
        const entries = await backend.ledgerList(filterOf(values));
        setRows(entries);
        setError(null);
      } catch (e) {
        console.warn("[llm-share] ledger list 读取失败", e);
        setError(errorText(e));
        setRows(null);
      }
    },
    [backend],
  );

  // 借用入账事件联动重拉：按当前生效过滤条件重查，最新 ref 避免 effect 闭包过期
  const latest = useRef({ query, filters });
  useEffect(() => {
    latest.current = { query, filters };
  });
  useEffect(
    () =>
      subscribeLedgerMutated(() => {
        void latest.current.query(latest.current.filters);
      }),
    [],
  );

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const entries = await backend.ledgerList({});
        if (!cancelled) {
          setRows(entries);
          setError(null);
        }
      } catch (e) {
        console.warn("[llm-share] ledger list 读取失败", e);
        if (!cancelled) setError(errorText(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [backend]);

  const set = (field: keyof FilterValues) => (value: string) =>
    setFilters((v) => ({ ...v, [field]: value }));

  return (
    <Card data-testid="ledger-entries">
      <CardHeader>
        <CardTitle className="text-sm">{t("llmShare.ledger.listTitle")}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <div className="grid grid-cols-3 items-end gap-3">
          <div className="flex flex-col gap-1">
            <Label htmlFor="llm-ledger-lender">{t("llmShare.ledger.filterLender")}</Label>
            <EntityCombobox
              id="llm-ledger-lender"
              testId="llm-ledger-lender"
              options={filterOptions}
              value={filters.lender || null}
              onChange={(v) => set("lender")(v ?? "")}
            />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="llm-ledger-borrower">{t("llmShare.ledger.filterBorrower")}</Label>
            <EntityCombobox
              id="llm-ledger-borrower"
              testId="llm-ledger-borrower"
              options={filterOptions}
              value={filters.borrower || null}
              onChange={(v) => set("borrower")(v ?? "")}
            />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="llm-ledger-period">{t("llmShare.ledger.filterPeriod")}</Label>
            <Input id="llm-ledger-period" value={filters.period} onChange={(e) => set("period")(e.target.value)} />
          </div>
        </div>
        <div>
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() => {
              void query(filters);
              onQueried?.();
            }}
          >
            {t("llmShare.ledger.applyFilter")}
          </Button>
        </div>
        {error ? <p role="alert" className="text-destructive text-xs">{error}</p> : null}
        {rows === null ? null : rows.length === 0 ? (
          <p className="text-muted-foreground text-sm">{t("llmShare.ledger.emptyList")}</p>
        ) : (
          <>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("llmShare.ledger.columnReqId")}</TableHead>
                  <TableHead>{t("llmShare.ledger.columnPeriod")}</TableHead>
                  <TableHead>{t("llmShare.ledger.columnLender")}</TableHead>
                  <TableHead>{t("llmShare.ledger.columnBorrower")}</TableHead>
                  <TableHead>{t("llmShare.ledger.columnModel")}</TableHead>
                  <TableHead>{t("llmShare.ledger.columnTokens")}</TableHead>
                  <TableHead>{t("llmShare.ledger.columnEstimated")}</TableHead>
                  <TableHead>{t("llmShare.ledger.columnTs")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map((entry) => (
                  <EntryRow key={entry.reqId} entry={entry} />
                ))}
              </TableBody>
            </Table>
            <p className="text-muted-foreground text-xs" data-testid="ledger-stats">
              {t("llmShare.ledger.statsLine", {
                count: rows.length,
                tokens: rows.reduce((sum, entry) => sum + entry.tokens, 0),
              })}
            </p>
          </>
        )}
      </CardContent>
    </Card>
  );
}
