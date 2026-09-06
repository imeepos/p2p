import type { I18nKey } from "@/i18n/types";

import type { LlmOfferPublishReq } from "./types";

// 表单契约显性化（§16.1 必填集）：models ≥1、spare 覆盖全部 model 且 N>0、
// period-ends 日期。GUI 表单层先行校验并逐字段提示，IPC 层仍兜底校验。
export interface OfferFormValues {
  modelsText: string;
  spareText: string;
  periodEnds: string;
  maxPerReqText: string;
  rpm: string;
  concurrency: string;
  ttl: string;
  retention: string;
}

export const EMPTY_OFFER_FORM: OfferFormValues = {
  modelsText: "",
  spareText: "",
  periodEnds: "",
  maxPerReqText: "",
  rpm: "10",
  concurrency: "2",
  ttl: "3600",
  retention: "none",
};

export type OfferField = "models" | "spare" | "periodEnds" | "maxPerReq" | "limits";
export type OfferErrors = Partial<Record<OfferField, I18nKey>>;

const KEY = "llmShare.offer";
const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

export function parseModels(text: string): string[] {
  return text
    .split(/[,\n，、]/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

type KvParse = { map: Record<string, number> } | { error: I18nKey };

export function parsePositiveKv(text: string): KvParse {
  const trimmed = text.trim();
  if (!trimmed) return { map: {} };
  const map: Record<string, number> = {};
  for (const line of trimmed.split("\n")) {
    const t = line.trim();
    if (!t) continue;
    const eq = t.indexOf("=");
    const key = eq > 0 ? t.slice(0, eq).trim() : "";
    const value = eq > 0 ? Number(t.slice(eq + 1).trim()) : Number.NaN;
    if (!key || !Number.isFinite(value)) {
      return { error: `${KEY}.errKvFormat` as I18nKey };
    }
    if (!Number.isInteger(value) || value <= 0) {
      return { error: `${KEY}.errPositiveInt` as I18nKey };
    }
    map[key] = value;
  }
  return { map: map };
}

function foreignKeyOf(map: Record<string, number>, models: string[]): string | null {
  for (const key of Object.keys(map)) {
    if (!models.includes(key)) return key;
  }
  return null;
}

function validateSpare(text: string, models: string[]): { error?: I18nKey; map?: Record<string, number> } {
  const parsed = parsePositiveKv(text);
  if ("error" in parsed) return { error: parsed.error };
  if (foreignKeyOf(parsed.map, models) !== null) {
    return { error: `${KEY}.errSpareForeign` as I18nKey };
  }
  if (models.some((m) => parsed.map[m] === undefined)) {
    return { error: `${KEY}.errSpareCoverage` as I18nKey };
  }
  return { map: parsed.map };
}

function validateMaxPerReq(text: string, models: string[]): { error?: I18nKey; map?: Record<string, number> } {
  const parsed = parsePositiveKv(text);
  if ("error" in parsed) return { error: parsed.error };
  if (foreignKeyOf(parsed.map, models) !== null) {
    return { error: `${KEY}.errMaxPerReqForeign` as I18nKey };
  }
  return { map: parsed.map };
}

function validatePositiveField(text: string): { error?: I18nKey; value?: number } {
  const trimmed = text.trim();
  if (!trimmed) return {};
  const n = Number(trimmed);
  if (!Number.isInteger(n) || n <= 0) return { error: `${KEY}.errPositiveInt` as I18nKey };
  return { value: n };
}

export interface OfferValidation {
  errors: OfferErrors;
  req: LlmOfferPublishReq | null;
}

export function validateOfferForm(values: OfferFormValues): OfferValidation {
  const errors: OfferErrors = {};
  const models = parseModels(values.modelsText);
  if (models.length === 0) errors.models = `${KEY}.errModelsRequired` as I18nKey;
  const spare =
    models.length === 0
      ? { map: {} as Record<string, number> }
      : validateSpare(values.spareText, models);
  if (spare.error) errors.spare = spare.error;
  const maxPerReq = validateMaxPerReq(values.maxPerReqText, models);
  if (maxPerReq.error) errors.maxPerReq = maxPerReq.error;
  if (!DATE_RE.test(values.periodEnds.trim())) {
    errors.periodEnds = (values.periodEnds.trim()
      ? `${KEY}.errPeriodFormat`
      : `${KEY}.errPeriodRequired`) as I18nKey;
  }
  const rpm = validatePositiveField(values.rpm);
  const concurrency = validatePositiveField(values.concurrency);
  const ttl = validatePositiveField(values.ttl);
  if (rpm.error) errors.limits = rpm.error;
  if (!errors.limits && concurrency.error) errors.limits = concurrency.error;
  if (!errors.limits && ttl.error) errors.limits = ttl.error;
  const req: LlmOfferPublishReq | null =
    Object.keys(errors).length > 0
      ? null
      : {
          models,
          spare: spare.map ?? {},
          periodEnds: values.periodEnds.trim(),
          maxPerReq: maxPerReq.map && Object.keys(maxPerReq.map).length > 0 ? maxPerReq.map : undefined,
          rpm: rpm.value,
          concurrency: concurrency.value,
          ttlSecs: ttl.value,
          retention: values.retention.trim() || undefined,
        };
  return { errors, req };
}
