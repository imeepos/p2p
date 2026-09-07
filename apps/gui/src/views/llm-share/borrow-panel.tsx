import { useEffect, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";

import type { I18nKey } from "@/i18n/types";

import { EntityCombobox, type PickerOption } from "@/components/picker";
import { useConfirm } from "@/components/feedback/confirm-provider";
import { toastError } from "@/components/feedback/toast";
import { CommandErrorText } from "@/components/feedback/command-error";
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
import { focusFirstInvalidField } from "./focus-first-error";
import { PeerIdField } from "./peer-id-field";
import { isValidFriendPeerId } from "@/views/contacts/chat-friend-rules";
import { consumeBorrowPrefill } from "./borrow-prefill";
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
  const [targetTouched, setTargetTouched] = useState(false);
  // W4：预填源 = 出借方 offer 快照（shareRedeem 应答内嵌，借方本地 allowlist 为空）
  // + query peer/model（防刷新重放：消费即清）。无快照时降级自由输入不阻塞。
  const [modelOptions, setModelOptions] = useState<PickerOption[]>([]);
  const [searchParams, setSearchParams] = useSearchParams();

  useEffect(() => {
    const stored = consumeBorrowPrefill();
    const peerParam = searchParams.get("peer");
    const modelParam = searchParams.get("model");
    if (!peerParam && !modelParam && !stored) return;
    if (peerParam || modelParam) {
      const next = new URLSearchParams(searchParams);
      next.delete("peer");
      next.delete("model");
      setSearchParams(next, { replace: true });
    }
    setValues((v) => ({
      ...v,
      targetPeer: peerParam ?? stored?.peer ?? v.targetPeer,
      model: modelParam ?? stored?.model ?? v.model,
    }));
    setModelOptions(
      (stored?.models ?? []).map((model) => ({ value: model, label: model })),
    );
  }, [searchParams, setSearchParams]);

  const set = (field: keyof BorrowFormValues) => (value: string) =>
    setValues((v) => ({ ...v, [field]: value }));

  // R2-05：出借方 PeerId 失焦即时格式校验（提交校验在 validateBorrowForm）
  const targetPeerErrorKey: I18nKey | null =
    errors.targetPeer ??
    (targetTouched &&
    values.targetPeer.trim().length > 0 &&
    !isValidFriendPeerId(values.targetPeer.trim())
      ? "llmShare.borrow.errTargetPeerFormat"
      : null);

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
      const text = errorText(error);
      setSubmitError(text);
      toastError(t("llmShare.borrow.submitFailed"), {
        description: text,
        context: "llm.borrow",
      });
    } finally {
      setBusy(false);
    }
  };

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault();
    const validation = validateBorrowForm(values);
    setErrors(validation.errors);
    // R2-12：校验失败聚焦第一个错误字段（与 aria-invalid 同源判定）
    const fieldErrors: Partial<Record<string, unknown>> = {
      "llm-borrow-peer": validation.errors.targetPeer,
      "llm-borrow-model": validation.errors.model,
      "llm-borrow-maxtokens": validation.errors.maxTokens,
      "llm-borrow-messages": validation.errors.messages,
    };
    focusFirstInvalidField(Object.keys(fieldErrors), (id) => fieldErrors[id] != null);
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
            <PeerIdField
              label={t("llmShare.borrow.formTargetPeer")}
              inputId="llm-borrow-peer"
              value={values.targetPeer}
              onValueChange={set("targetPeer")}
              onBlur={() => setTargetTouched(true)}
              errorKey={targetPeerErrorKey}
              errorId="llm-borrow-peer-error"
              placeholder={t("llmShare.borrow.formTargetPeerPlaceholder")}
            />
            <div className="flex flex-col gap-1">
              <Label htmlFor="llm-borrow-model-pick">{t("llmShare.borrow.formModelPick")}</Label>
              <EntityCombobox
                id="llm-borrow-model-pick"
                testId="llm-borrow-model-pick"
                options={modelOptions}
                value={modelOptions.some((option) => option.value === values.model) ? values.model : null}
                onChange={(next) => {
                  set("model")(next ?? "");
                }}
                placeholder={t("llmShare.borrow.modelPickPlaceholder")}
                searchPlaceholder={t("llmShare.borrow.modelPickSearch")}
                emptyText={t("llmShare.borrow.modelPickEmpty")}
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="flex flex-col gap-1">
                <Label htmlFor="llm-borrow-model">{t("llmShare.borrow.formModel")}</Label>
                <Input
                  id="llm-borrow-model"
                  value={values.model}
                  onChange={(e) => set("model")(e.target.value)}
                  aria-invalid={errors.model ? true : undefined}
                  aria-describedby={errors.model ? "llm-borrow-model-error" : undefined}
                />
                {errors.model ? (
                  <p role="alert" id="llm-borrow-model-error" className="text-destructive text-xs">
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
                  placeholder={t("llmShare.borrow.formMaxTokensPlaceholder")}
                  aria-describedby={
                    errors.maxTokens ? "llm-borrow-maxtokens-error" : "llm-borrow-maxtokens-hint"
                  }
                  aria-invalid={errors.maxTokens ? true : undefined}
                />
                <p id="llm-borrow-maxtokens-hint" className="text-muted-foreground text-xs">
                  {t("llmShare.borrow.formMaxTokensHint")}
                </p>
                {errors.maxTokens ? (
                  <p role="alert" id="llm-borrow-maxtokens-error" className="text-destructive text-xs">
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
                placeholder={t("llmShare.borrow.formMessagesPlaceholder")}
                aria-describedby={
                  errors.messages ? "llm-borrow-messages-error" : "llm-borrow-messages-hint"
                }
                aria-invalid={errors.messages ? true : undefined}
              />
              <p id="llm-borrow-messages-hint" className="text-muted-foreground text-xs">
                {t("llmShare.borrow.formMessagesHint")}
              </p>
              {errors.messages ? (
                <p role="alert" id="llm-borrow-messages-error" className="text-destructive text-xs">
                  {t(errors.messages)}
                </p>
              ) : null}
            </div>
            {submitError ? (
              <CommandErrorText message={submitError} />
            ) : null}
            {/* R2-14：LLM 调用为长耗时动作，处理中给行内状态反馈（aria-live） */}
            {busy ? (
              <p role="status" aria-live="polite" className="text-muted-foreground text-xs" data-testid="borrow-calling">
                {t("llmShare.borrow.calling")}
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