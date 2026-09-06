import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { errorText } from "@/views/shared/form-flow";
import { StatusBadge } from "@/views/shared/status-badge";

import type { LlmReceiptVerifyResult, LlmShareBackend } from "./types";

// 收据验核入口（§16.1 receipt verify）：缺省本机身份仅出借方自验；
// 借方场景传出借方公钥 —— lenderPubkey 为高级字段，默认折叠。
export function ReceiptVerifyCard({ backend }: { backend: LlmShareBackend }) {
  const { t } = useTranslation();
  const [reqId, setReqId] = useState("");
  const [lenderPubkey, setLenderPubkey] = useState("");
  const [advanced, setAdvanced] = useState(false);
  const [result, setResult] = useState<LlmReceiptVerifyResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const handleVerify = async () => {
    setBusy(true);
    setError(null);
    try {
      const pubkey = advanced && lenderPubkey.trim() ? lenderPubkey.trim() : undefined;
      const verdict = await backend.receiptVerify({ reqId: reqId.trim(), lenderPubkey: pubkey });
      setResult(verdict);
    } catch (e) {
      console.error("[llm-share] receipt verify 失败", e);
      setError(errorText(e));
      setResult(null);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card data-testid="receipt-verify">
      <CardHeader>
        <CardTitle className="text-sm">{t("llmShare.ledger.verifyTitle")}</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <div className="flex flex-col gap-1">
          <Label htmlFor="llm-verify-reqid">{t("llmShare.ledger.verifyReqId")}</Label>
          <Input
            id="llm-verify-reqid"
            value={reqId}
            onChange={(e) => setReqId(e.target.value)}
            className="font-mono"
          />
        </div>
        <div>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            onClick={() => setAdvanced((v) => !v)}
            aria-expanded={advanced}
          >
            {t("llmShare.ledger.advancedToggle")}
          </Button>
        </div>
        {advanced ? (
          <div className="flex flex-col gap-1">
            <Label htmlFor="llm-verify-pubkey">{t("llmShare.ledger.advancedLenderPubkey")}</Label>
            <Input
              id="llm-verify-pubkey"
              value={lenderPubkey}
              onChange={(e) => setLenderPubkey(e.target.value)}
              className="font-mono"
            />
          </div>
        ) : null}
        {error ? (
          <p role="alert" className="text-destructive text-xs">
            {error}
          </p>
        ) : null}
        <div>
          <Button type="button" size="sm" disabled={busy} onClick={() => void handleVerify()}>
            {t("llmShare.ledger.verify")}
          </Button>
        </div>
        {result ? (
          <div
            data-testid="verify-result"
            data-verdict={result.verdict}
            className="rounded-md border p-3 text-xs"
          >
            <div className="flex items-center gap-2">
              <StatusBadge tone={result.verdict === "PASS" ? "success" : "danger"} dot>
                {result.verdict === "PASS"
                  ? t("llmShare.ledger.verdictPass")
                  : t("llmShare.ledger.verdictFail")}
              </StatusBadge>
              <span className="font-mono">{result.reqId}</span>
            </div>
            <p className="mt-1">
              {t("llmShare.ledger.reasonLabel")}: {result.reason}
            </p>
            {result.verdict === "PASS" && result.model ? (
              <p className="text-muted-foreground mt-1">
                {result.period} {result.lender} {result.model} {result.input}/{result.output}
              </p>
            ) : null}
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}
