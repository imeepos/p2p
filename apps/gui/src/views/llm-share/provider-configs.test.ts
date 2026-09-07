import { afterEach, describe, expect, it, vi } from "vitest";

import {
  EMPTY_PROVIDER_FORM,
  EMPTY_SHARE_FORM,
  loadProviderConfigs,
  maskApiKey,
  newProviderId,
  parseProviderForm,
  removeProviderConfig,
  saveProviderConfigs,
  sparePrefillOf,
  upsertProviderConfig,
  validateShareForm,
  type ProviderConfig,
} from "./provider-configs";

const MODELS = ["deepseek-chat", "deepseek-reasoner"];
const FRIEND = "52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd";

afterEach(() => {
  localStorage.clear();
  vi.restoreAllMocks();
});

describe("provider 配置表单校验（本地自用登记）", () => {
  it("空表单逐字段报错且不出配置", () => {
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

  it("合法输入产出去空格配置与模型清单", () => {
    const { errors, config } = parseProviderForm({
      name: "  DeepSeek 官方 ",
      baseUrl: " https://api.deepseek.com/v1 ",
      apiKey: " sk-abc ",
      modelsText: "deepseek-chat, deepseek-reasoner",
    });
    expect(errors).toEqual({});
    expect(config).toEqual({
      name: "DeepSeek 官方",
      baseUrl: "https://api.deepseek.com/v1",
      apiKey: "sk-abc",
      models: ["deepseek-chat", "deepseek-reasoner"],
    });
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

describe("分享给好友表单校验（offerPublish 请求产出）", () => {
  it("空分享表单：peerId/闲量/账期逐项报错", () => {
    const { errors, publishReq, peerId } = validateShareForm(EMPTY_SHARE_FORM, MODELS);
    expect(publishReq).toBeNull();
    expect(peerId).toBeNull();
    expect(errors.peerId).toBe("llmShare.providers.shareErrPeerRequired");
    expect(errors.spare).toBe("llmShare.providers.shareErrSpareCoverage");
    expect(errors.periodEnds).toBe("llmShare.providers.shareErrPeriodRequired");
  });

  it("闲量未覆盖全部模型报覆盖错，非正整数复用 offer 表单 KV 错", () => {
    const partial = validateShareForm(
      { ...EMPTY_SHARE_FORM, spareText: "deepseek-chat=100" },
      MODELS,
    );
    expect(partial.errors.spare).toBe("llmShare.providers.shareErrSpareCoverage");
    const zero = validateShareForm(
      { ...EMPTY_SHARE_FORM, spareText: "deepseek-chat=0\ndeepseek-reasoner=1" },
      MODELS,
    );
    expect(zero.errors.spare).toBe("llmShare.offer.errPositiveInt");
  });

  it("非法 PeerId 报格式错", () => {
    const { errors } = validateShareForm(
      { ...EMPTY_SHARE_FORM, peerId: "not-a-peer" },
      MODELS,
    );
    expect(errors.peerId).toBe("llmShare.providers.shareErrPeerFormat");
  });

  it("合法输入产出按配置模型发布的请求与放行 peerId", () => {
    const { errors, publishReq, peerId } = validateShareForm(
      {
        peerId: FRIEND,
        spareText: "deepseek-chat=1500000\ndeepseek-reasoner=900000",
        periodEnds: "2026-09-30",
        note: " 首月互借 ",
      },
      MODELS,
    );
    expect(errors).toEqual({});
    expect(peerId).toBe(FRIEND);
    expect(publishReq).toEqual({
      models: MODELS,
      spare: { "deepseek-chat": 1500000, "deepseek-reasoner": 900000 },
      periodEnds: "2026-09-30",
      rpm: 10,
      concurrency: 2,
      ttlSecs: 3600,
      retention: "none",
    });
  });

  it("sparePrefillOf 逐模型出 model= 行，数值留空待填", () => {
    expect(sparePrefillOf(MODELS)).toBe("deepseek-chat=\ndeepseek-reasoner=");
  });
});

describe("localStorage 存取（仅本机，损坏显式告警不静默）", () => {
  it("save→load 往返一致", () => {
    const config: ProviderConfig = {
      id: "pv-1",
      name: "DeepSeek 官方",
      baseUrl: "https://api.deepseek.com/v1",
      apiKey: "sk-test-1234567890abcd",
      models: MODELS,
      createdAt: 1,
    };
    saveProviderConfigs([config]);
    expect(loadProviderConfigs()).toEqual([config]);
  });

  it("损坏存档告警回空列表", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    localStorage.setItem("p2p-gui-llm-providers", "{not-json");
    expect(loadProviderConfigs()).toEqual([]);
    expect(warn).toHaveBeenCalled();
  });

  it("形状不符的条目被剔除而非崩溃", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    localStorage.setItem(
      "p2p-gui-llm-providers",
      JSON.stringify([{ id: "x" }, { id: "pv-2", name: "n", baseUrl: "https://a", apiKey: "k", models: ["m"], createdAt: 2 }]),
    );
    const loaded = loadProviderConfigs();
    expect(loaded).toHaveLength(1);
    expect(loaded[0].id).toBe("pv-2");
    expect(warn).not.toHaveBeenCalled();
  });

  it("upsert 同 id 覆盖不重复；remove 删目标 id", () => {
    const a: ProviderConfig = { id: "pv-1", name: "A", baseUrl: "https://a", apiKey: "k", models: ["m"], createdAt: 1 };
    const a2 = { ...a, name: "A2" };
    const b: ProviderConfig = { id: "pv-2", name: "B", baseUrl: "https://b", apiKey: "k", models: ["m"], createdAt: 2 };
    const upserted = upsertProviderConfig([a], b);
    expect(upserted.map((c) => c.id)).toEqual(["pv-1", "pv-2"]);
    expect(upsertProviderConfig(upserted, a2).map((c) => c.name)).toEqual(["A2", "B"]);
    expect(removeProviderConfig(upserted, "pv-1").map((c) => c.id)).toEqual(["pv-2"]);
  });

  it("newProviderId 唯一", () => {
    expect(newProviderId()).not.toBe(newProviderId());
  });
});
