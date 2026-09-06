import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

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
import { errorText } from "@/views/shared/form-flow";
import { StatusBadge } from "@/views/shared/status-badge";

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

function EntryRow({ entry }: { entry: LlmLedgerEntry }) {
  const { t } = useTranslation();
  return (
    <TableRow data-testid="ledger-row">
      <TableCell className="max-w-36 truncate font-mono text-xs" title={entry.reqId}>
        {entry.reqId}
      </TableCell>
      <TableCell className="text-xs">{entry.period}</TableCell>
      <TableCell className="max-w-28 truncate font-mono text-xs" title={entry.lender}>
        {entry.lender}
      </TableCell>
      <TableCell className="max-w-28 truncate font-mono text-xs" title={entry.borrower}>
        {entry.borrower}
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
    </TableRow>
  );
}

export function LedgerEntriesCard({ backend }: { backend: LlmShareBackend }) {
  const { t } = useTranslation();
  const [filters, setFilters] = useState<FilterValues>(EMPTY_FILTER);
  const [rows, setRows] = useState<LlmLedgerEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

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
            <Input id="llm-ledger-lender" value={filters.lender} onChange={(e) => set("lender")(e.target.value)} />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="llm-ledger-borrower">{t("llmShare.ledger.filterBorrower")}</Label>
            <Input id="llm-ledger-borrower" value={filters.borrower} onChange={(e) => set("borrower")(e.target.value)} />
          </div>
          <div className="flex flex-col gap-1">
            <Label htmlFor="llm-ledger-period">{t("llmShare.ledger.filterPeriod")}</Label>
            <Input id="llm-ledger-period" value={filters.period} onChange={(e) => set("period")(e.target.value)} />
          </div>
        </div>
        <div>
          <Button type="button" size="sm" variant="outline" onClick={() => void query(filters)}>
            {t("llmShare.ledger.applyFilter")}
          </Button>
        </div>
        {error ? <p role="alert" className="text-destructive text-xs">{error}</p> : null}
        {rows === null ? null : rows.length === 0 ? (
          <p className="text-muted-foreground text-sm">{t("llmShare.ledger.emptyList")}</p>
        ) : (
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
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.map((entry) => (
                <EntryRow key={entry.reqId} entry={entry} />
              ))}
            </TableBody>
          </Table>
        )}
      </CardContent>
    </Card>
  );
}
