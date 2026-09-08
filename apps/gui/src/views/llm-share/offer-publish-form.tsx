import { ChevronDownIcon, Loader2Icon, PlusIcon } from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";

import type { I18nKey } from "@/i18n/types";

import { toastError, toastSuccess } from "@/components/feedback/toast";
import { CommandErrorText } from "@/components/feedback/command-error";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { errorText } from "@/views/shared/form-flow";

import { focusFirstInvalidField } from "./focus-first-error";
import {
  datePlusDays,
  EMPTY_OFFER_FORM,
  offerToForm,
  parseModels,
  spareRowsOf,
  validateOfferForm,
  type OfferErrors,
  type OfferFormValues,
} from "./offer-form";
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
    "llm-offer-spare-0": errors.spare,
    "llm-offer-period": errors.periodEnds,
    "llm-offer-maxperreq": errors.maxPerReq,
    "llm-offer-rpm": errors.limits,
  };
}

// 「能选择的不手输」：ProviderStore 的模型一键带入声明（v13 迁移后不再读 localStorage
// 明文键），未收录的仍可手输；候选源读取失败不阻塞发布（告警信号即可）。
function providerModelCandidates(current: string[], providerModels: string[]): string[] {
  const included = new Set(current);
  return [...new Set(providerModels)].filter((m) => !included.has(m));
}

function QuickAddChips({
  modelsText,
  providerModels,
  onAdd,
}: {
  modelsText: string;
  providerModels: string[];
  onAdd: (model: string) => void;
}) {
  const { t } = useTranslation();
  const candidates = providerModelCandidates(parseModels(modelsText), providerModels);
  if (candidates.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-1" data-testid="offer-model-quickadd">
      {candidates.map((model) => (
        <button
          key={model}
          type="button"
          onClick={() => onAdd(model)}
          aria-label={t("llmShare.offer.quickAddAria", { model })}
          data-testid={"offer-quickadd-" + model}
          className="hover:bg-accent inline-flex cursor-pointer items-center gap-0.5 rounded-md border px-1.5 py-0.5 text-xs"
        >
          <PlusIcon aria-hidden className="size-3" />
          {model}
        </button>
      ))}
    </div>
  );
}

function SpareRows({
  values,
  error,
  onChange,
}: {
  values: OfferFormValues;
  error?: I18nKey;
  onChange: (model: string, value: string) => void;
}) {
  const { t } = useTranslation();
  const rows = spareRowsOf(values);
  if (rows.length === 0) return null;
  return (
    <div className="flex flex-col gap-2">
      <Label>{t("llmShare.offer.formSpare")}</Label>
      {rows.map((row, index) => (
        <div key={row.model} className="flex items-center gap-2">
          <Label
            htmlFor={`llm-offer-spare-${index}`}
            className="w-40 shrink-0 truncate font-mono text-xs"
            title={row.model}
          >
            {row.model}
          </Label>
          <Input
            id={`llm-offer-spare-${index}`}
            inputMode="numeric"
            value={row.value}
            onChange={(e) => onChange(row.model, e.target.value)}
            placeholder={t("llmShare.offer.spareRowPlaceholder")}
            className="font-mono text-xs"
            aria-invalid={error ? true : undefined}
            aria-describedby={error ? "llm-offer-spare-error" : undefined}
          />
        </div>
      ))}
      <p className="text-muted-foreground text-xs">{t("llmShare.offer.spareRowsHint")}</p>
      <FieldError messageKey={error} htmlId="llm-offer-spare-error" />
    </div>
  );
}

const PERIOD_PRESETS = [7, 30, 90] as const;

function PeriodField({
  value,
  error,
  onChange,
}: {
  value: string;
  error?: I18nKey;
  onChange: (value: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-1">
      <Label htmlFor="llm-offer-period">{t("llmShare.offer.formPeriodEnds")}</Label>
      <div className="flex items-center gap-2">
        <Input
          id="llm-offer-period"
          type="date"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? "llm-offer-period-error" : undefined}
        />
        {PERIOD_PRESETS.map((days) => (
          <Button
            key={days}
            type="button"
            size="sm"
            variant="outline"
            className="shrink-0"
            onClick={() => onChange(datePlusDays(days))}
            data-testid={`offer-period-preset-${days}d`}
          >
            {t(`llmShare.offer.preset${days}d`)}
          </Button>
        ))}
      </div>
      <FieldError messageKey={error} htmlId="llm-offer-period-error" />
    </div>
  );
}

function AdvancedFields({
  values,
  errors,
  set,
}: {
  values: OfferFormValues;
  errors: OfferErrors;
  set: (field: keyof OfferFormValues) => (value: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-3 rounded-md border border-dashed p-3">
      <div className="flex flex-col gap-1">
        <Label htmlFor="llm-offer-maxperreq">{t("llmShare.offer.formMaxPerReq")}</Label>
        <Textarea
          id="llm-offer-maxperreq"
          rows={2}
          value={values.maxPerReqText}
          onChange={(e) => set("maxPerReqText")(e.target.value)}
          placeholder={t("llmShare.offer.formMaxPerReqPlaceholder")}
          aria-invalid={errors.maxPerReq ? true : undefined}
          aria-describedby={errors.maxPerReq ? "llm-offer-maxperreq-error" : undefined}
        />
        <FieldError messageKey={errors.maxPerReq} htmlId="llm-offer-maxperreq-error" />
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
      <FieldError messageKey={errors.limits} htmlId="llm-offer-limits-error" />
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
    </div>
  );
}

interface OfferPublishFormProps {
  /** 已发布声明（更新场景）：打开即回填，改个数就能重发 */
  offer: LlmOfferView | null;
  backend: LlmShareBackend;
  onPublished: (offer: LlmOfferView) => void;
  onCancel: () => void;
}

// 能力发布表单（从 offer-panel 拆出）：面板本体只留状态卡与入口按钮，
// 表单在点击「发布/更新」时才出现。新建缺省账期 +30 天，限流组有厂值缺省，
// 低频高级字段默认折叠——常规发布只需 模型 + 各模型闲量 + 签名发布。
export function OfferPublishForm({ offer, backend, onPublished, onCancel }: OfferPublishFormProps) {
  const { t } = useTranslation();
  const [values, setValues] = useState<OfferFormValues>(() =>
    offer
      ? offerToForm(offer)
      : { ...EMPTY_OFFER_FORM, periodEnds: datePlusDays(30) },
  );
  const [errors, setErrors] = useState<OfferErrors>({});
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [publishError, setPublishError] = useState<string | null>(null);
  // v13：模型快捷候选来自 ProviderStore（迁移后 localStorage 不再持有配置）
  const [providerModels, setProviderModels] = useState<string[]>([]);

  useEffect(() => {
    let cancelled = false;
    void backend
      .providerList()
      .then(({ providers }) => {
        if (!cancelled) setProviderModels([...new Set(providers.flatMap((p) => p.models))]);
      })
      .catch((error) => {
        // 候选源读取失败不阻塞发布：留告警信号
        console.warn("[llm-share] provider 模型候选读取失败", error);
      });
    return () => {
      cancelled = true;
    };
  }, [backend]);

  const set = (field: keyof OfferFormValues) => (value: string) =>
    setValues((v) => ({ ...v, [field]: value }));

  const setSpare = (model: string, value: string) =>
    setValues((v) => ({ ...v, spare: { ...v.spare, [model]: value } }));

  const addModel = (model: string) =>
    setValues((v) => {
      const models = parseModels(v.modelsText);
      if (models.includes(model)) return v;
      return { ...v, modelsText: [...models, model].join(", ") };
    });

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
      // R2-08：保存类操作对齐全站 toast 反馈，不再仅状态卡静默出现
      toastSuccess(t("llmShare.offer.publishSuccess"));
      onPublished(view);
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
    <form
      className="flex flex-col gap-3 rounded-md border p-3"
      onSubmit={(e) => void handleSubmit(e)}
      noValidate
      data-testid="offer-publish-form"
    >
      <div className="flex flex-col gap-1">
        <Label htmlFor="llm-offer-models">{t("llmShare.offer.formModels")}</Label>
        <Input
          id="llm-offer-models"
          value={values.modelsText}
          onChange={(e) => set("modelsText")(e.target.value)}
          placeholder={t("llmShare.offer.formModelsPlaceholder")}
          aria-invalid={errors.models ? true : undefined}
          aria-describedby={errors.models ? "llm-offer-models-error" : "llm-offer-models-hint"}
        />
        <p id="llm-offer-models-hint" className="text-muted-foreground text-xs">
          {t("llmShare.offer.formModelsHint")}
        </p>
        <FieldError messageKey={errors.models} htmlId="llm-offer-models-error" />
        <QuickAddChips modelsText={values.modelsText} providerModels={providerModels} onAdd={addModel} />
      </div>
      <SpareRows
        values={values}
        error={errors.spare}
        onChange={setSpare}
      />
      <PeriodField value={values.periodEnds} error={errors.periodEnds} onChange={set("periodEnds")} />
      <button
        type="button"
        className="flex w-fit items-center gap-1 text-xs"
        onClick={() => setAdvancedOpen((open) => !open)}
        aria-expanded={advancedOpen}
        data-testid="offer-advanced-toggle"
      >
        <ChevronDownIcon
          aria-hidden
          className={`size-3.5 transition-transform ${advancedOpen ? "rotate-180" : ""}`}
        />
        {t("llmShare.offer.advancedToggle")}
      </button>
      {advancedOpen ? <AdvancedFields values={values} errors={errors} set={set} /> : null}
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
        <Button type="button" size="sm" variant="outline" disabled={busy} onClick={onCancel}>
          {t("common.actions.cancel")}
        </Button>
      </div>
    </form>
  );
}