import type { I18nKey } from "@/i18n/types";

import { parseModels } from "./offer-form";
import type { LlmProviderProtocol, LlmProviderSaveReq } from "./types";

// provider 配置视图模型（契约 §16.6 v13）：存储归属从 localStorage 上移为后端
// ProviderStore（providers.json + 0600 密钥文件）；本文件只做迁移源读取与表单校验。
// apiKey：新建/编辑时临时持有明文；列表加载态为空串，展示走 apiKeyMasked（后端掩码）。
// protocol：openai | claude，旧存档缺省 openai（校验兼容）。

export interface ProviderConfig {
  id: string;
  name: string;
  baseUrl: string;
  protocol: LlmProviderProtocol;
  apiKey: string;
  apiKeyMasked?: string;
  models: string[];
  createdAt: number;
}

export interface ProviderFormValues {
  name: string;
  baseUrl: string;
  protocol: LlmProviderProtocol;
  apiKey: string;
  modelsText: string;
}

export const EMPTY_PROVIDER_FORM: ProviderFormValues = {
  name: "",
  baseUrl: "",
  protocol: "openai",
  apiKey: "",
  modelsText: "",
};

export type ProviderField = "name" | "baseUrl" | "apiKey" | "models";
export type ProviderErrors = Partial<Record<ProviderField, I18nKey>>;

const KEY = "llmShare.providers";
export const PROVIDER_STORAGE_KEY = "p2p-gui-llm-providers";

export function newProviderId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  console.warn("[llm-share] crypto.randomUUID 缺席，使用随机串生成 providerId");
  return "pv-" + Math.random().toString(36).slice(2) + Date.now().toString(36);
}

export interface ProviderParseResult {
  errors: ProviderErrors;
  config: Omit<ProviderConfig, "id" | "createdAt"> | null;
}

// apiKeyOptional=true（编辑已有配置）：留空 = 保留原密钥（后端更新语义），仅新增必填。
export function parseProviderForm(
  values: ProviderFormValues,
  opts?: { apiKeyOptional?: boolean },
): ProviderParseResult {
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
  if (!apiKey && !opts?.apiKeyOptional) errors.apiKey = `${KEY}.errApiKeyRequired` as I18nKey;
  if (models.length === 0) errors.models = `${KEY}.errModelsRequired` as I18nKey;
  const config =
    Object.keys(errors).length > 0
      ? null
      : { name, baseUrl, protocol: values.protocol, apiKey, models };
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

/** http:// baseUrl 显式告警（§7 安全红线：明文传输 API Key，https 优先） */
export function baseUrlHttpWarningKey(baseUrl: string): I18nKey | null {
  try {
    const url = new URL(baseUrl.trim());
    return url.protocol === "http:" ? "llmShare.providers.baseUrlHttpWarning" : null;
  } catch {
    return null;
  }
}

// apiKey 只以掩码出现在列表里，防旁观泄露；过短键全遮。
export function maskApiKey(apiKey: string): string {
  const trimmed = apiKey.trim();
  if (trimmed.length < 8) return "••••••••";
  return `${trimmed.slice(0, 3)}••••${trimmed.slice(-4)}`;
}

// ---- localStorage 迁移源（契约 §16.6-1：一次性幂等迁移后 removeItem 旧键） ----

export function loadProviderConfigs(): ProviderConfig[] {
  try {
    const raw = localStorage.getItem(PROVIDER_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) throw new Error("providers 存档不是数组");
    const configs = parsed
      .map(normalizeLegacyProvider)
      .filter((c): c is ProviderConfig => c !== null);
    if (configs.length !== parsed.length) {
      console.warn("[llm-share] provider 存档含形状不符条目，已剔除");
    }
    return configs;
  } catch (error) {
    console.warn("[llm-share] provider 配置存档不可读，回空列表", error);
    return [];
  }
}

/** 旧存档归一：protocol 缺省 openai；形状不符返回 null（校验兼容不崩溃） */
export function normalizeLegacyProvider(value: unknown): ProviderConfig | null {
  if (typeof value !== "object" || value === null) return null;
  const v = value as Partial<ProviderConfig>;
  if (
    typeof v.id !== "string" ||
    typeof v.name !== "string" ||
    typeof v.baseUrl !== "string" ||
    typeof v.apiKey !== "string" ||
    !Array.isArray(v.models) ||
    v.models.some((m) => typeof m !== "string") ||
    typeof v.createdAt !== "number"
  ) {
    return null;
  }
  return {
    id: v.id,
    name: v.name,
    baseUrl: v.baseUrl,
    protocol: v.protocol === "claude" ? "claude" : "openai",
    apiKey: v.apiKey,
    models: [...v.models],
    createdAt: v.createdAt,
  };
}

/** 一次性幂等迁移：读取旧键逐条 providerSave（同 id upsert 幂等）→ 成功 removeItem；
 *  失败保留旧键可重试（半迁移不丢源）；迁移后写路径不再落明文键。 */
export async function migrateLegacyProvidersToStore(backend: {
  providerSave: (req: LlmProviderSaveReq) => Promise<unknown>;
}): Promise<void> {
  try {
    const configs = loadProviderConfigs();
    if (configs.length === 0) return;
    for (const config of configs) {
      await backend.providerSave({
        id: config.id,
        name: config.name,
        baseUrl: config.baseUrl,
        protocol: config.protocol,
        apiKey: config.apiKey,
        models: config.models,
      });
    }
    localStorage.removeItem(PROVIDER_STORAGE_KEY);
  } catch (error) {
    // 失败回滚：不删旧键（保留重试源），显式告警不静默
    console.warn("[llm-share] provider 存档迁移失败，保留旧键待重试", error);
  }
}
