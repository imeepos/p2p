import type { I18nKey } from "@/i18n/types";

import type { LlmOfferPublishReq, LlmOfferView } from "./types";

// 表单契约显性化（§16.1 必填集）：models ≥1、spare 覆盖全部 model 且 N>0、
// period-ends 日期。GUI 表单层先行校验并逐字段提示，IPC 层仍兜底校验。
// spare 由「每行一条 model=N」文本升级为按模型逐行结构化输入：键即已声明
// 模型，从源头杜绝键名拼错与漏行两类高频错误。
export interface OfferFormValues {
  modelsText: string;
  /** 每模型闲量结构化行：键 = 已声明模型，值 = 正整数字符串（空 = 未填） */
  spare: Record<string, string>;
  periodEnds: string;
  maxPerReqText: string;
  rpm: string;
  concurrency: string;
  ttl: string;
  retention: string;
}

export const EMPTY_OFFER_FORM: OfferFormValues = {
  modelsText: "",
  spare: {},
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

/** 本地时区 from+n 天的 YYYY-MM-DD（账期快捷预设 / 新建表单缺省） */
export function datePlusDays(days: number, from = new Date()): string {
  const d = new Date(from.getFullYear(), from.getMonth(), from.getDate() + days);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** 从已发布 offer 回填表单（「更新声明」免重输） */
export function offerToForm(offer: LlmOfferView): OfferFormValues {
  const spare: Record<string, string> = {};
  for (const model of offer.models) spare[model] = "";
  for (const [model, n] of Object.entries(offer.spare)) spare[model] = String(n);
  return {
    modelsText: offer.models.join(", "),
    spare,
    periodEnds: offer.periodEnds,
    maxPerReqText: offer.maxPerReq
      ? Object.entries(offer.maxPerReq)
          .map(([m, n]) => `${m}=${n}`)
          .join("\n")
      : "",
    rpm: offer.rpm != null ? String(offer.rpm) : EMPTY_OFFER_FORM.rpm,
    concurrency:
      offer.concurrency != null ? String(offer.concurrency) : EMPTY_OFFER_FORM.concurrency,
    ttl: String(offer.ttlSecs),
    retention: offer.retention ?? EMPTY_OFFER_FORM.retention,
  };
}

/** 已声明模型的闲量行（保持声明顺序）；模型增删时已填值跟随保留 */
export function spareRowsOf(values: OfferFormValues): { model: string; value: string }[] {
  return parseModels(values.modelsText).map((model) => ({
    model,
    value: values.spare[model] ?? "",
  }));
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

function validateMaxPerReq(
  text: string,
  models: string[],
): { error?: I18nKey; map?: Record<string, number> } {
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

function validateSpareRows(
  models: string[],
  rows: Record<string, string>,
): { error?: I18nKey; map?: Record<string, number> } {
  const map: Record<string, number> = {};
  for (const model of models) {
    const raw = (rows[model] ?? "").trim();
    if (!raw) return { error: `${KEY}.errSpareRequired` as I18nKey };
    const n = Number(raw);
    if (!Number.isInteger(n) || n <= 0) return { error: `${KEY}.errPositiveInt` as I18nKey };
    map[model] = n;
  }
  return { map };
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
      : validateSpareRows(models, values.spare);
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
          maxPerReq:
            maxPerReq.map && Object.keys(maxPerReq.map).length > 0 ? maxPerReq.map : undefined,
          rpm: rpm.value,
          concurrency: concurrency.value,
          ttlSecs: ttl.value,
          retention: values.retention.trim() || undefined,
        };
  return { errors, req };
}
