import type { I18nKey } from "@/i18n/types";

import { isValidFriendPeerId } from "@/views/contacts/chat-friend-rules";

import { parseModels, parsePositiveKv } from "./offer-form";
import type { LlmOfferPublishReq } from "./types";

// 本地自用的上游 provider 配置列表（GUI-local，localStorage 存档）：
// 一条配置 = 名称 + OpenAI 兼容 baseUrl + apiKey + 模型清单。
// 存档语义沿用 acp/endpoint-storage：损坏显式告警回空白，绝不静默。
// 契约 §16.2-4 禁 GUI 直写账本/offer 文件——本列表只进 localStorage，
// 密钥只存本机；「分享配置」分享的是 allowlist 放行 + 能力声明，非密钥本体。

export interface ProviderConfig {
  id: string;
  name: string;
  baseUrl: string;
  apiKey: string;
  models: string[];
  createdAt: number;
}

export interface ProviderFormValues {
  name: string;
  baseUrl: string;
  apiKey: string;
  modelsText: string;
}

export const EMPTY_PROVIDER_FORM: ProviderFormValues = {
  name: "",
  baseUrl: "",
  apiKey: "",
  modelsText: "",
};

export type ProviderField = "name" | "baseUrl" | "apiKey" | "models";
export type ProviderErrors = Partial<Record<ProviderField, I18nKey>>;

const KEY = "llmShare.providers";
const STORAGE_KEY = "p2p-gui-llm-providers";

export function newProviderId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  console.warn("[llm-share] crypto.randomUUID 缺席，使用随机串生成 providerId");
  return "pv-" + Math.random().toString(36).slice(2) + Date.now().toString(36);
}

export function parseProviderForm(values: ProviderFormValues): {
  errors: ProviderErrors;
  config: Omit<ProviderConfig, "id" | "createdAt"> | null;
} {
  const errors: ProviderErrors = {};
  const name = values.name.trim();
  const baseUrl = values.baseUrl.trim();
  const apiKey = values.apiKey.trim();
  const models = parseModels(values.modelsText);
  if (!name) errors.name = `${KEY}.errNameRequired` as I18nKey;
  if (!baseUrl) {
    errors.baseUrl = `${KEY}.errBaseUrlRequired` as I18nKey;
  } else if (!isHttpUrl(baseUrl)) {
    errors.baseUrl = `${KEY}.errBaseUrlFormat` as I18nKey;
  }
  if (!apiKey) errors.apiKey = `${KEY}.errApiKeyRequired` as I18nKey;
  if (models.length === 0) errors.models = `${KEY}.errModelsRequired` as I18nKey;
  const config =
    Object.keys(errors).length > 0 ? null : { name, baseUrl, apiKey, models };
  return { errors, config };
}

function isHttpUrl(text: string): boolean {
  try {
    const url = new URL(text);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}

// apiKey 只以掩码出现在列表里，防旁观泄露；过短键全遮。
export function maskApiKey(apiKey: string): string {
  const trimmed = apiKey.trim();
  if (trimmed.length < 8) return "••••••••";
  return `${trimmed.slice(0, 3)}••••${trimmed.slice(-4)}`;
}

// ---- 分享给好友：peerId + spare/period 校验，产出 offerPublish 请求 ----

export interface ShareFormValues {
  peerId: string;
  spareText: string;
  periodEnds: string;
  note: string;
}

export const EMPTY_SHARE_FORM: ShareFormValues = {
  peerId: "",
  spareText: "",
  periodEnds: "",
  note: "",
};

export type ShareField = "peerId" | "spare" | "periodEnds";
export type ShareErrors = Partial<Record<ShareField, I18nKey>>;

const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

export interface ShareValidation {
  errors: ShareErrors;
  peerId: string | null;
  publishReq: LlmOfferPublishReq | null;
}

// 分享 = 按配置模型发布能力声明 + 把好友按同批模型放行（allow 由调用方组装）。
// spare 逐模型必填（覆盖校验与 offer 表单同口径），密钥不参与任何请求。
export function validateShareForm(
  values: ShareFormValues,
  models: string[],
): ShareValidation {
  const errors: ShareErrors = {};
  const peerId = values.peerId.trim();
  if (!peerId) {
    errors.peerId = `${KEY}.shareErrPeerRequired` as I18nKey;
  } else if (!isValidFriendPeerId(peerId)) {
    errors.peerId = `${KEY}.shareErrPeerFormat` as I18nKey;
  }
  const parsed = parsePositiveKv(values.spareText);
  const spare = "map" in parsed ? parsed.map : {};
  if ("error" in parsed) {
    // KV 形状/正整数错复用 offer 表单同款错误键（同一解析器）
    errors.spare = parsed.error;
  } else if (models.some((m) => spare[m] === undefined)) {
    errors.spare = `${KEY}.shareErrSpareCoverage` as I18nKey;
  }
  if (!DATE_RE.test(values.periodEnds.trim())) {
    errors.periodEnds = `${KEY}.shareErrPeriodRequired` as I18nKey;
  }
  const ok = Object.keys(errors).length === 0;
  return {
    errors,
    peerId: ok ? peerId : null,
    publishReq: ok
      ? {
          models,
          spare,
          periodEnds: values.periodEnds.trim(),
          rpm: 10,
          concurrency: 2,
          ttlSecs: 3600,
          retention: "none",
        }
      : null,
  };
}

// 分享表单闲量预填：逐模型出 model= 行，数值留空待填（不替用户编造额度）。
export function sparePrefillOf(models: string[]): string {
  return models.map((m) => `${m}=`).join("\n");
}

// ---- localStorage 存取（仅本机，进不了任何共享面） ----

export function loadProviderConfigs(): ProviderConfig[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) throw new Error("providers 存档不是数组");
    return parsed.filter(isProviderConfig);
  } catch (error) {
    console.warn("[llm-share] provider 配置存档不可读，回空列表", error);
    return [];
  }
}

export function saveProviderConfigs(configs: ProviderConfig[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(configs));
  } catch (error) {
    console.warn("[llm-share] provider 配置存档不可写，仅本次生效", error);
  }
}

function isProviderConfig(value: unknown): value is ProviderConfig {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Partial<ProviderConfig>;
  return (
    typeof v.id === "string" &&
    typeof v.name === "string" &&
    typeof v.baseUrl === "string" &&
    typeof v.apiKey === "string" &&
    Array.isArray(v.models) &&
    v.models.every((m) => typeof m === "string") &&
    typeof v.createdAt === "number"
  );
}

export function upsertProviderConfig(
  configs: ProviderConfig[],
  config: ProviderConfig,
): ProviderConfig[] {
  const rest = configs.filter((c) => c.id !== config.id);
  return [...rest, config].sort((a, b) => a.createdAt - b.createdAt);
}

export function removeProviderConfig(
  configs: ProviderConfig[],
  id: string,
): ProviderConfig[] {
  return configs.filter((c) => c.id !== id);
}
