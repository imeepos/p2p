import { useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";

import { useConfirm } from "@/components/feedback/confirm-provider";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { errorText } from "@/views/shared/form-flow";

import {
  EMPTY_BORROW_FORM,
  newReqId,
  validateBorrowForm,
  type BorrowErrors,
  type BorrowFormValues,
} from "./borrow-form";
import { BorrowReportCard } from "./borrow-report";
import { notifyLedgerMutated } from "./ledger-sync";
import type { LlmBorrowReq, LlmBorrowReport, LlmShareBackend } from "./types";

// borrow 快捷面板：提交前二次确认对话框明示真实成本（§16.2-6）；
// reqId 于首次提交生成、重试复用同值（§16.2-3），「新请求」才重置。
export function BorrowPanel({ backend }: { backend: LlmShareBackend }) {
  const { t } = useTranslation();
  const confirm = useConfirm();
  const [values, setValues] = useState<BorrowFormValues>(EMPTY_BORROW_FORM);
  const [errors, setErrors] = useState<BorrowErrors>({});
  const [report, setReport] = useState<LlmBorrowReport | null>(null);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [hasIntent, setHasIntent] = useState(false);
  const [reqId, setReqId] = useState<string | null>(null);
  const [lastReq, setLastReq] = useState<LlmBorrowReq | null>(null);

  const set = (field: keyof BorrowFormValues) => (value: string) =>
    setValues((v) => ({ ...v, [field]: value }));

  const runBorrow = async (req: LlmBorrowReq) => {
    const ok = await confirm({
      title: t("llmShare.borrow.confirmTitle"),
      description: `${t("llmShare.borrow.confirmBody")} ${t(
        "llmShare.borrow.costLine",
        { model: req.model, maxTokens: req.maxTokens, peer: req.targetPeer },
      )}`,
      confirmText: t("common.actions.confirm"),
      cancelText: t("common.actions.cancel"),
    });
    if (!ok) return;
    setBusy(true);
    setSubmitError(null);
    try {
      const result = await backend.borrow(req);
      setReport(result);
      // R2-01：真实入账（含估算入账）即广播，净差/流水卡联动重拉
      if (result.receipt.appended) notifyLedgerMutated();
    } catch (error) {
      console.error("[llm-share] borrow 失败", error);
      setSubmitError(errorText(error));
    } finally {
      setBusy(false);
    }
  };

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault();
    const validation = validateBorrowForm(values);
    setErrors(validation.errors);
    if (!validation.req) return;
    const id = reqId ?? newReqId();
    if (id !== reqId) setReqId(id);
    const req: LlmBorrowReq = { ...validation.req, reqId: id };
    setLastReq(req);
    setHasIntent(true);
    await runBorrow(req);
  };

  const handleRetry = async () => {
    if (!lastReq) return;
    await runBorrow(lastReq);
  };

  const handleReset = () => {
    setReqId(null);
    setLastReq(null);
    setHasIntent(false);
    setReport(null);
    setSubmitError(null);
    setErrors({});
    setValues(EMPTY_BORROW_FORM);
  };

  return (
    <div className="flex flex-col gap-3" data-testid="borrow-panel">
      <Card>
        <CardHeader>
          <CardTitle className="text-sm">{t("llmShare.panels.borrow")}</CardTitle>
        </CardHeader>
        <CardContent>
          <form className="flex flex-col gap-3" onSubmit={(e) => void handleSubmit(e)} noValidate>
            <div className="flex flex-col gap-1">
              <Label htmlFor="llm-borrow-peer">{t("llmShare.borrow.formTargetPeer")}</Label>
              <Input
                id="llm-borrow-peer"
                value={values.targetPeer}
                onChange={(e) => set("targetPeer")(e.target.value)}
                placeholder={t("llmShare.borrow.formTargetPeerPlaceholder")}
              />
              {errors.targetPeer ? (
                <p role="alert" className="text-destructive text-xs">
                  {t(errors.targetPeer)}
                </p>
              ) : null}
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="flex flex-col gap-1">
                <Label htmlFor="llm-borrow-model">{t("llmShare.borrow.formModel")}</Label>
                <Input
                  id="llm-borrow-model"
                  value={values.model}
                  onChange={(e) => set("model")(e.target.value)}
                />
                {errors.model ? (
                  <p role="alert" className="text-destructive text-xs">
                    {t(errors.model)}
                  </p>
                ) : null}
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="llm-borrow-maxtokens">{t("llmShare.borrow.formMaxTokens")}</Label>
                <Input
                  id="llm-borrow-maxtokens"
                  inputMode="numeric"
                  value={values.maxTokensText}
                  onChange={(e) => set("maxTokensText")(e.target.value)}
                />
                {errors.maxTokens ? (
                  <p role="alert" className="text-destructive text-xs">
                    {t(errors.maxTokens)}
                  </p>
                ) : null}
              </div>
            </div>
            <div className="flex flex-col gap-1">
              <Label htmlFor="llm-borrow-messages">{t("llmShare.borrow.formMessages")}</Label>
              <Textarea
                id="llm-borrow-messages"
                rows={3}
                value={values.messages}
                onChange={(e) => set("messages")(e.target.value)}
              />
              {errors.messages ? (
                <p role="alert" className="text-destructive text-xs">
                  {t(errors.messages)}
                </p>
              ) : null}
            </div>
            {submitError ? (
              <p role="alert" className="text-destructive text-xs">
                {submitError}
              </p>
            ) : null}
            <div className="flex gap-2">
              <Button type="submit" size="sm" disabled={busy}>
                {t("llmShare.borrow.submit")}
              </Button>
              {hasIntent ? (
                <Button type="button" size="sm" variant="outline" disabled={busy} onClick={() => void handleRetry()}>
                  {t("llmShare.borrow.retry")}
                </Button>
              ) : null}
              {hasIntent ? (
                <Button type="button" size="sm" variant="ghost" onClick={handleReset}>
                  {t("llmShare.borrow.reset")}
                </Button>
              ) : null}
            </div>
          </form>
        </CardContent>
      </Card>
      {report ? <BorrowReportCard report={report} /> : null}
    </div>
  );
}
