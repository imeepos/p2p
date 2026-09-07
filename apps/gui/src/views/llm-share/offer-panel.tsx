import { CircleSlash } from "lucide-react";
import { useCallback, useEffect, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";

import type { I18nKey } from "@/i18n/types";

import { toastSuccess } from "@/components/feedback/toast";
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
import { OfferStatusCard } from "./offer-status-card";
import type { LlmOfferView, LlmShareBackend } from "./types";

function FieldError({ messageKey }: { messageKey?: I18nKey }) {
  const { t } = useTranslation();
  if (!messageKey) return null;
  return (
    <p role="alert" className="text-destructive text-xs">
      {t(messageKey)}
    </p>
  );
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
      setPublishError(errorText(error));
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
              />
              <FieldError messageKey={errors.models} />
            </div>
            <div className="flex flex-col gap-1">
              <Label htmlFor="llm-offer-spare">{t("llmShare.offer.formSpare")}</Label>
              <Textarea
                id="llm-offer-spare"
                rows={3}
                value={values.spareText}
                onChange={(e) => set("spareText")(e.target.value)}
                placeholder={t("llmShare.offer.formSparePlaceholder")}
              />
              <FieldError messageKey={errors.spare} />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="flex flex-col gap-1">
                <Label htmlFor="llm-offer-period">{t("llmShare.offer.formPeriodEnds")}</Label>
                <Input
                  id="llm-offer-period"
                  type="date"
                  value={values.periodEnds}
                  onChange={(e) => set("periodEnds")(e.target.value)}
                />
                <FieldError messageKey={errors.periodEnds} />
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="llm-offer-maxperreq">{t("llmShare.offer.formMaxPerReq")}</Label>
                <Input
                  id="llm-offer-maxperreq"
                  value={values.maxPerReqText}
                  onChange={(e) => set("maxPerReqText")(e.target.value)}
                  placeholder={t("llmShare.offer.formMaxPerReqPlaceholder")}
                />
                <FieldError messageKey={errors.maxPerReq} />
              </div>
            </div>
            <div className="grid grid-cols-3 gap-3">
              <div className="flex flex-col gap-1">
                <Label htmlFor="llm-offer-rpm">{t("llmShare.offer.formRpm")}</Label>
                <Input id="llm-offer-rpm" value={values.rpm} onChange={(e) => set("rpm")(e.target.value)} />
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
            <FieldError messageKey={errors.limits} />
            {publishError ? (
              <p role="alert" className="text-destructive text-xs">
                {t("llmShare.offer.publishFailed")}: {publishError}
              </p>
            ) : null}
            <div className="flex gap-2">
              <Button type="submit" size="sm" disabled={busy}>
                {t("llmShare.offer.publish")}
              </Button>
              <Button type="button" size="sm" variant="outline" onClick={() => void refresh()}>
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
