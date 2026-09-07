import { CircleSlash, Loader2Icon } from "lucide-react";
import { useCallback, useEffect, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";

import type { I18nKey } from "@/i18n/types";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import { CommandErrorText } from "@/components/feedback/command-error";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { errorText } from "@/views/shared/form-flow";
import { EmptyState } from "@/views/shared/empty-state";

import {
  EMPTY_OFFER_FORM,
  validateOfferForm,
  type OfferErrors,
  type OfferFormValues,
} from "./offer-form";
import { isOfferNotPublished, warnOfferLoadOnce } from "./offer-errors";
import { focusFirstInvalidField } from "./focus-first-error";
import { OfferStatusCard } from "./offer-status-card";
import type { LlmOfferView, LlmShareBackend } from "./types";

function FieldError({ messageKey, htmlId }: { messageKey?: I18nKey; htmlId?: string }) {
  const { t } = useTranslation();
  if (!messageKey) return null;
  return (
    <p role="alert" id={htmlId} className="text-destructive text-xs">
      {t(messageKey)}
    </p>
  );
}

// R2-12：字段 id → 错误键映射（聚焦顺序 = 表单视觉顺序；limits 归属 RPM 输入）
function fieldErrorOf(errors: OfferErrors): Partial<Record<string, I18nKey>> {
  return {
    "llm-offer-models": errors.models,
    "llm-offer-spare": errors.spare,
    "llm-offer-period": errors.periodEnds,
    "llm-offer-maxperreq": errors.maxPerReq,
    "llm-offer-rpm": errors.limits,
  };
}

export function OfferPanel({ backend }: { backend: LlmShareBackend }) {
  const { t } = useTranslation();
  const [values, setValues] = useState<OfferFormValues>(EMPTY_OFFER_FORM);
  const [errors, setErrors] = useState<OfferErrors>({});
  const [offer, setOffer] = useState<LlmOfferView | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [publishError, setPublishError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // R2-03：未发布是常态非故障——空态静默走中文出路文案；仅真实错误才
  // 原样露出并告警（会话级单次，console 不逐次刷屏）。
  const reportLoadError = useCallback((error: unknown) => {
    if (isOfferNotPublished(error)) {
      setLoadError(null);
      return;
    }
    warnOfferLoadOnce(error);
    setLoadError(errorText(error));
  }, []);

  const refresh = useCallback(async () => {
    try {
      const view = await backend.offerShow();
      setOffer(view);
      setLoadError(null);
    } catch (error) {
      reportLoadError(error);
      setOffer(null);
    }
  }, [backend, reportLoadError]);

  // 挂载拉取走 effect 内联 IIFE（react-hooks/set-state-in-effect 合规形态，
  // use-gui-config 先例）；refresh 供按钮手动刷新复用。
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const view = await backend.offerShow();
        if (!cancelled) {
          setOffer(view);
          setLoadError(null);
        }
      } catch (error) {
        if (!cancelled) reportLoadError(error);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [backend, reportLoadError]);

  const set = (field: keyof OfferFormValues) => (value: string) =>
    setValues((v) => ({ ...v, [field]: value }));

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault();
    const validation = validateOfferForm(values);
    setErrors(validation.errors);
    // R2-12：校验失败聚焦第一个错误字段（与 aria-invalid 同源判定）
    const fieldErrors = fieldErrorOf(validation.errors);
    focusFirstInvalidField(Object.keys(fieldErrors), (id) => fieldErrors[id] != null);
    if (!validation.req) return;
    setBusy(true);
    setPublishError(null);
    try {
      const view = await backend.offerPublish(validation.req);
      setOffer(view);
      setLoadError(null);
      // R2-08：保存类操作对齐全站 toast 反馈，不再仅状态卡静默出现
      toastSuccess(t("llmShare.offer.publishSuccess"));
    } catch (error) {
      console.error("[llm-share] offer publish 失败", error);
      const text = errorText(error);
      setPublishError(text);
      toastError(t("llmShare.offer.publishFailed"), {
        description: text,
        context: "llm.offer_publish",
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-3" data-testid="offer-panel">
      <Card>
        <CardHeader>
          <CardTitle className="text-sm">{t("llmShare.panels.offer")}</CardTitle>
        </CardHeader>
        <CardContent>
          <form className="flex flex-col gap-3" onSubmit={(e) => void handleSubmit(e)} noValidate>
            <div className="flex flex-col gap-1">
              <Label htmlFor="llm-offer-models">{t("llmShare.offer.formModels")}</Label>
              <Input
                id="llm-offer-models"
                value={values.modelsText}
                onChange={(e) => set("modelsText")(e.target.value)}
                placeholder={t("llmShare.offer.formModelsPlaceholder")}
                aria-invalid={errors.models ? true : undefined}
                aria-describedby={errors.models ? "llm-offer-models-error" : undefined}
              />
              <FieldError messageKey={errors.models} htmlId="llm-offer-models-error" />
            </div>
            <div className="flex flex-col gap-1">
              <Label htmlFor="llm-offer-spare">{t("llmShare.offer.formSpare")}</Label>
              <Textarea
                id="llm-offer-spare"
                rows={3}
                value={values.spareText}
                onChange={(e) => set("spareText")(e.target.value)}
                placeholder={t("llmShare.offer.formSparePlaceholder")}
                aria-invalid={errors.spare ? true : undefined}
                aria-describedby={errors.spare ? "llm-offer-spare-error" : undefined}
              />
              <FieldError messageKey={errors.spare} htmlId="llm-offer-spare-error" />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="flex flex-col gap-1">
                <Label htmlFor="llm-offer-period">{t("llmShare.offer.formPeriodEnds")}</Label>
                <Input
                  id="llm-offer-period"
                  type="date"
                  value={values.periodEnds}
                  onChange={(e) => set("periodEnds")(e.target.value)}
                  aria-invalid={errors.periodEnds ? true : undefined}
                  aria-describedby={errors.periodEnds ? "llm-offer-period-error" : undefined}
                />
                <FieldError messageKey={errors.periodEnds} htmlId="llm-offer-period-error" />
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="llm-offer-maxperreq">{t("llmShare.offer.formMaxPerReq")}</Label>
                <Input
                  id="llm-offer-maxperreq"
                  value={values.maxPerReqText}
                  onChange={(e) => set("maxPerReqText")(e.target.value)}
                  placeholder={t("llmShare.offer.formMaxPerReqPlaceholder")}
                  aria-invalid={errors.maxPerReq ? true : undefined}
                  aria-describedby={errors.maxPerReq ? "llm-offer-maxperreq-error" : undefined}
                />
                <FieldError messageKey={errors.maxPerReq} htmlId="llm-offer-maxperreq-error" />
              </div>
            </div>
            <div className="grid grid-cols-3 gap-3">
              <div className="flex flex-col gap-1">
                <Label htmlFor="llm-offer-rpm">{t("llmShare.offer.formRpm")}</Label>
                <Input
                  id="llm-offer-rpm"
                  value={values.rpm}
                  onChange={(e) => set("rpm")(e.target.value)}
                  aria-invalid={errors.limits ? true : undefined}
                  aria-describedby={errors.limits ? "llm-offer-limits-error" : undefined}
                />
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="llm-offer-concurrency">{t("llmShare.offer.formConcurrency")}</Label>
                <Input
                  id="llm-offer-concurrency"
                  value={values.concurrency}
                  onChange={(e) => set("concurrency")(e.target.value)}
                />
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="llm-offer-ttl">{t("llmShare.offer.formTtl")}</Label>
                <Input id="llm-offer-ttl" value={values.ttl} onChange={(e) => set("ttl")(e.target.value)} />
              </div>
            </div>
            {/* R2-15：留存自述渲染（可选输入 + 用途说明），此前键在而字段缺渲染 */}
            <div className="flex flex-col gap-1">
              <Label htmlFor="llm-offer-retention">{t("llmShare.offer.formRetention")}</Label>
              <Input
                id="llm-offer-retention"
                value={values.retention}
                onChange={(e) => set("retention")(e.target.value)}
                placeholder="none"
                aria-describedby="llm-offer-retention-hint"
              />
              <p id="llm-offer-retention-hint" className="text-muted-foreground text-xs">
                {t("llmShare.offer.formRetentionHint")}
              </p>
            </div>
            <FieldError messageKey={errors.limits} htmlId="llm-offer-limits-error" />
            {publishError ? (
              <CommandErrorText
                message={publishError}
                prefix={t("llmShare.offer.publishFailed") + "："}
              />
            ) : null}
            <div className="flex gap-2">
              <Button type="submit" size="sm" disabled={busy}>
                {busy ? <Loader2Icon aria-hidden className="size-4 animate-spin" /> : null}
                {busy ? t("llmShare.offer.publishing") : t("llmShare.offer.publish")}
              </Button>
              <Button type="button" size="sm" variant="outline" disabled={busy} onClick={() => void refresh()}>
                {t("llmShare.offer.refresh")}
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
      {offer ? (
        <OfferStatusCard offer={offer} />
      ) : (
        <EmptyState
          icon={CircleSlash}
          title={t("llmShare.offer.emptyTitle")}
          description={loadError ?? t("llmShare.offer.emptyHint")}
        />
      )}
    </div>
  );
}
