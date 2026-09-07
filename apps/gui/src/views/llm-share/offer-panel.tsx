import { CircleSlash } from "lucide-react";
import { useCallback, useEffect, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";

import type { I18nKey } from "@/i18n/types";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { errorText } from "@/views/shared/form-flow";
import { EmptyState } from "@/views/shared/empty-state";
import { StatusBadge, type StatusTone } from "@/views/shared/status-badge";

import {
  EMPTY_OFFER_FORM,
  validateOfferForm,
  type OfferErrors,
  type OfferFormValues,
} from "./offer-form";
import { isOfferNotPublished } from "./offer-errors";
import type { LlmOfferStatus, LlmOfferView, LlmShareBackend } from "./types";

// §16.2-5：expired/not_yet_valid = 常态中性；peer_mismatch/bad_signature =
// 醒目警示（danger 徽章 + role=alert + destructive 描边）。
const STATUS_TONE: Record<LlmOfferStatus, StatusTone> = {
  live: "success",
  expired: "neutral",
  not_yet_valid: "neutral",
  peer_mismatch: "danger",
  bad_signature: "danger",
};

const STATUS_KEY: Record<LlmOfferStatus, I18nKey> = {
  live: "llmShare.offer.statusLive",
  expired: "llmShare.offer.statusExpired",
  not_yet_valid: "llmShare.offer.statusNotYetValid",
  peer_mismatch: "llmShare.offer.statusPeerMismatch",
  bad_signature: "llmShare.offer.statusBadSignature",
};

function FieldError({ messageKey }: { messageKey?: I18nKey }) {
  const { t } = useTranslation();
  if (!messageKey) return null;
  return (
    <p role="alert" className="text-destructive text-xs">
      {t(messageKey)}
    </p>
  );
}

function OfferFieldRow({ label, value }: { label: string; value: string }) {
  return (
    <>
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="truncate font-mono" title={value}>
        {value}
      </dd>
    </>
  );
}

export function OfferStatusCard({ offer }: { offer: LlmOfferView }) {
  const { t } = useTranslation();
  const warning = offer.status === "peer_mismatch" || offer.status === "bad_signature";
  const spare = Object.entries(offer.spare)
    .map(([m, n]) => `${m}=${n}`)
    .join(", ");
  return (
    <div
      data-testid="offer-status"
      data-status={offer.status}
      className={
        warning
          ? "border-destructive/40 rounded-md border p-3 text-sm"
          : "rounded-md border p-3 text-sm"
      }
    >
      <div className="flex items-center gap-2">
        <StatusBadge tone={STATUS_TONE[offer.status]} dot>
          {t(STATUS_KEY[offer.status])}
        </StatusBadge>
        <span className="text-muted-foreground text-xs">
          {t("llmShare.offer.labelStatus")}
        </span>
      </div>
      {warning ? (
        <p role="alert" className="text-destructive mt-2 text-xs">
          {t("llmShare.offer.securityWarning")}
        </p>
      ) : null}
      <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-xs">
        <OfferFieldRow label={t("llmShare.offer.labelPeer")} value={offer.peer} />
        <OfferFieldRow
          label={t("llmShare.offer.labelModels")}
          value={offer.models.join(", ")}
        />
        <OfferFieldRow label={t("llmShare.offer.labelSpare")} value={spare} />
        <OfferFieldRow
          label={t("llmShare.offer.labelPeriodEnds")}
          value={offer.periodEnds}
        />
        <OfferFieldRow
          label={t("llmShare.offer.labelRemaining")}
          value={String(offer.remainingSecs)}
        />
      </dl>
    </div>
  );
}

// R2-26：console 降噪——未发布空态静默，真实错误整个会话仅提示一次
let loadWarned = false;

/** 测试专用：重置会话级告警标记（模块单例用例隔离入口，resetToastDedupForTest 惯例） */
export function resetOfferLoadWarnForTest(): void {
  loadWarned = false;
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
    if (!loadWarned) {
      loadWarned = true;
      console.warn("[llm-share] offer show 失败", error);
    }
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
