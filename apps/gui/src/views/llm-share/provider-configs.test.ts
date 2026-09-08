import { afterEach, describe, expect, it, vi } from "vitest";

import {
  baseUrlHttpWarningKey,
  EMPTY_PROVIDER_FORM,
  loadProviderConfigs,
  maskApiKey,
  migrateLegacyProvidersToStore,
  newProviderId,
  normalizeLegacyProvider,
  parseProviderForm,
  PROVIDER_STORAGE_KEY,
  type ProviderConfig,
} from "./provider-configs";
import { makeLlmShareMockPair } from "./mock-backend";

const MODELS = ["deepseek-chat", "deepseek-reasoner"];

function legacyRaw(configs: unknown[]): void {
  localStorage.setItem(PROVIDER_STORAGE_KEY, JSON.stringify(configs));
}

afterEach(() => {
  localStorage.clear();
  vi.restoreAllMocks();
});

describe("provider 表单校验（协议字段 + 存档兼容）", () => {
  it("空表单逐字段报错且不出配置；协议缺省 openai", () => {
    const { errors, config } = parseProviderForm(EMPTY_PROVIDER_FORM);
    expect(config).toBeNull();
    expect(errors.name).toBe("llmShare.providers.errNameRequired");
    expect(errors.baseUrl).toBe("llmShare.providers.errBaseUrlRequired");
    expect(errors.apiKey).toBe("llmShare.providers.errApiKeyRequired");
    expect(errors.models).toBe("llmShare.providers.errModelsRequired");
  });

  it("baseUrl 非 http(s) 报格式错，ftp 与纯文本都拦下", () => {
    const base = { ...EMPTY_PROVIDER_FORM, name: "x", apiKey: "k", modelsText: "m1" };
    for (const bad of ["ftp://x", "api.deepseek.com/v1"]) {
      const { errors, config } = parseProviderForm({ ...base, baseUrl: bad });
      expect(config).toBeNull();
      expect(errors.baseUrl).toBe("llmShare.providers.errBaseUrlFormat");
    }
  });

  it("合法输入产出去空格配置并带协议字段", () => {
    const { errors, config } = parseProviderForm({
      name: "  DeepSeek 官方 ",
      baseUrl: " https://api.deepseek.com/v1 ",
      protocol: "claude",
      apiKey: " sk-abc ",
      modelsText: "deepseek-chat, deepseek-reasoner",
    });
    expect(errors).toEqual({});
    expect(config).toEqual({
      name: "DeepSeek 官方",
      baseUrl: "https://api.deepseek.com/v1",
      protocol: "claude",
      apiKey: "sk-abc",
      models: ["deepseek-chat", "deepseek-reasoner"],
    });
  });

  it("编辑态 apiKey 可留空（保留原密钥）；新增态留空报错", () => {
    const base = { ...EMPTY_PROVIDER_FORM, name: "x", baseUrl: "https://a", modelsText: "m1" };
    const created = parseProviderForm(base);
    expect(created.errors.apiKey).toBe("llmShare.providers.errApiKeyRequired");
    const edited = parseProviderForm(base, { apiKeyOptional: true });
    expect(edited.errors.apiKey).toBeUndefined();
    expect(edited.config?.apiKey).toBe("");
  });
});

describe("baseUrl http 显式告警（§7 安全红线）", () => {
  it("http:// 出告警键；https 与非法输入无告警", () => {
    expect(baseUrlHttpWarningKey("http://api.example.com")).toBe(
      "llmShare.providers.baseUrlHttpWarning",
    );
    expect(baseUrlHttpWarningKey("https://api.example.com")).toBeNull();
    expect(baseUrlHttpWarningKey("not-a-url")).toBeNull();
    expect(baseUrlHttpWarningKey("")).toBeNull();
  });
});

describe("maskApiKey（密钥只以掩码出现）", () => {
  it("长键留头 3 尾 4，中段遮蔽", () => {
    expect(maskApiKey("sk-test-1234567890abcd")).toBe("sk-••••abcd");
  });

  it("过短键全遮不留痕", () => {
    expect(maskApiKey("abc")).toBe("••••••••");
  });
});

describe("旧存档读取（迁移源，仅本机）", () => {
  it("save→load 往返一致（含协议字段）", () => {
    const config: ProviderConfig = {
      id: "pv-1",
      name: "DeepSeek 官方",
      baseUrl: "https://api.deepseek.com/v1",
      protocol: "openai",
      apiKey: "sk-test-1234567890abcd",
      models: MODELS,
      createdAt: 1,
    };
    legacyRaw([config]);
    expect(loadProviderConfigs()).toEqual([config]);
  });

  it("旧存档缺 protocol 字段：兼容缺省 openai", () => {
    legacyRaw([
      {
        id: "pv-1",
        name: "n",
        baseUrl: "https://a",
        apiKey: "k",
        models: ["m"],
        createdAt: 1,
      },
    ]);
    const [loaded] = loadProviderConfigs();
    expect(loaded.protocol).toBe("openai");
    expect(normalizeLegacyProvider({ ...loaded, protocol: "claude" })?.protocol).toBe("claude");
  });

  it("损坏存档告警回空列表；形状不符条目剔除而非崩溃", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    localStorage.setItem(PROVIDER_STORAGE_KEY, "{not-json");
    expect(loadProviderConfigs()).toEqual([]);
    expect(warn).toHaveBeenCalled();
    legacyRaw([{ id: "x" }, { id: "pv-2", name: "n", baseUrl: "https://a", apiKey: "k", models: ["m"], createdAt: 2 }]);
    const loaded = loadProviderConfigs();
    expect(loaded).toHaveLength(1);
    expect(loaded[0].id).toBe("pv-2");
  });

  it("newProviderId 唯一", () => {
    expect(newProviderId()).not.toBe(newProviderId());
  });
});

describe("localStorage → ProviderStore 一次性幂等迁移（§16.6-1）", () => {
  it("迁移成功：条目进后端且旧键 removeItem，迁移后写路径不再落明文键", async () => {
    legacyRaw([
      { id: "pv-1", name: "A", baseUrl: "https://a", apiKey: "sk-aaa", models: ["m1"], createdAt: 1 },
      { id: "pv-2", name: "B", baseUrl: "https://b", apiKey: "sk-bbb", models: ["m2"], createdAt: 2 },
    ]);
    const { mock, backend } = makeLlmShareMockPair();
    await migrateLegacyProvidersToStore(backend);
    expect(localStorage.getItem(PROVIDER_STORAGE_KEY)).toBeNull();
    const providers = mock.providerList().providers;
    expect(providers.map((p) => p.id).sort()).toEqual(["pv-1", "pv-2"]);
    expect(providers.find((p) => p.id === "pv-1")?.protocol).toBe("openai");
    // 迁移后再调用：无旧键即 no-op，不重复写入
    const before = mock.providerList().providers.length;
    await migrateLegacyProvidersToStore(backend);
    expect(mock.providerList().providers).toHaveLength(before);
  });

  it("迁移失败：旧键保留可重试（不残留半迁移态），告警不静默", async () => {
    legacyRaw([
      { id: "pv-1", name: "A", baseUrl: "https://a", apiKey: "sk-aaa", models: ["m1"], createdAt: 1 },
    ]);
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const failing = {
      providerSave: () => Promise.reject(new Error("backend down")),
    };
    await migrateLegacyProvidersToStore(failing);
    expect(localStorage.getItem(PROVIDER_STORAGE_KEY)).not.toBeNull();
    expect(warn).toHaveBeenCalled();
  });
});
