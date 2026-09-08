import { describe, expect, it } from "vitest";

import { parseLlmShareLink } from "@/lib/llm-share-link-model";

import { LlmShareMock, LlmShareMockError, MOCK_LOCAL_PEER } from "./mock-backend";

const T0 = 1788549300;
const PROVIDER_MODELS = ["gpt-4o", "deepseek-v3"];

function seededMock(opts: { now?: () => number; redeemerPeerId?: string } = {}) {
  const mock = new LlmShareMock({ now: opts.now ?? (() => T0), redeemerPeerId: opts.redeemerPeerId });
  const provider = mock.providerSave({
    name: "P",
    baseUrl: "https://api.example.com/v1",
    protocol: "openai",
    apiKey: "sk-test-1234567890abcd",
    models: PROVIDER_MODELS,
  });
  mock.offerPublish({
    models: PROVIDER_MODELS,
    spare: { "gpt-4o": 1000, "deepseek-v3": 1000 },
    periodEnds: "2026-09-30",
  });
  return { mock, provider };
}

describe("v13 provider save/list/remove（契约 §16.6 B2 的 GUI mock 面）", () => {
  it("save 生成 id 并只回掩码；list 幂等；remove 级联不存在=显式报错", () => {
    const mock = new LlmShareMock({ now: () => T0 });
    const view = mock.providerSave({
      name: "P",
      baseUrl: "https://api.example.com/v1",
      protocol: "claude",
      apiKey: "sk-test-1234567890abcd",
      models: ["claude-3-5-sonnet"],
    });
    expect(view.id).toBeTruthy();
    expect(view.protocol).toBe("claude");
    expect(view.apiKeyMasked).not.toContain("sk-test");
    expect(mock.providerList().providers).toHaveLength(1);
    expect(() => mock.providerRemove("pv-missing")).toThrow(LlmShareMockError);
    expect(mock.providerRemove(view.id)).toEqual({ removed: true });
    expect(mock.providerList().providers).toHaveLength(0);
  });

  it("save 校验：name/baseUrl/models 必填；创建缺 apiKey 显式报错；更新留空保留密钥", () => {
    const mock = new LlmShareMock({ now: () => T0 });
    const base = { baseUrl: "https://a", protocol: "openai" as const, models: ["m"] };
    expect(() => mock.providerSave({ ...base, name: " ", apiKey: "k" })).toThrow(/name/);
    expect(() => mock.providerSave({ ...base, name: "n", baseUrl: " ", apiKey: "k" })).toThrow(/baseUrl/);
    expect(() => mock.providerSave({ ...base, name: "n", apiKey: "k", models: [] })).toThrow(/models/);
    expect(() => mock.providerSave({ ...base, name: "n", apiKey: "" })).toThrow(/apiKey/);
    const view = mock.providerSave({ ...base, name: "n", apiKey: "k" });
    const updated = mock.providerSave({ id: view.id, name: "n2", baseUrl: "https://a", protocol: "openai", models: ["m"] });
    expect(updated.name).toBe("n2");
    expect(updated.apiKeyMasked).toBe(view.apiKeyMasked);
  });
});

describe("v13 share 全链 roundtrip（§16.6 B4 的 GUI mock 面）", () => {
  it("shareCreate → link 解析 → redeem → allowlist 条目 source=share:<id> → borrow 可用", () => {
    const { mock } = seededMock();
    const created = mock.shareCreate({ providerId: mock.providerList().providers[0].id });
    const parsed = parseLlmShareLink(created.link);
    expect(parsed.peer).toBe(MOCK_LOCAL_PEER);
    expect(parsed.models).toEqual(PROVIDER_MODELS);
    expect(created.expiresAt).toBe(T0 + 24 * 3600);
    const result = mock.shareRedeem(created.link);
    expect(result.shareId).toBe(created.shareId);
    expect(result.offer.peer).toBe(MOCK_LOCAL_PEER);
    expect(result.offer.models).toEqual(PROVIDER_MODELS);
    const [entry] = mock.allowList().entries;
    expect(entry.models).toEqual(PROVIDER_MODELS);
    expect(entry.note).toBe(`share:${created.shareId}`);
    // 兑换落 allowlist 后 borrow 可结算（默认拒绝语义被显式放行替代）
    const report = mock.borrow({
      targetPeer: MOCK_LOCAL_PEER,
      model: "gpt-4o",
      messages: "hi",
      maxTokens: 8,
    });
    expect(report.status).toBe("done");
  });

  it("shareCreate 校验：模型须 ⊆ 当前 offer（空集与越界都拒）", () => {
    const { mock, provider } = seededMock();
    expect(() => mock.shareCreate({ providerId: provider.id, models: [] })).toThrow(/non-empty/);
    expect(() =>
      mock.shareCreate({ providerId: provider.id, models: ["not-in-offer"] }),
    ).toThrow(/not in current offer/);
  });

  it("同 peer 二次兑换幂等成功不重复计数；异 peer 拒绝 bound-other", () => {
    const { mock } = seededMock({ redeemerPeerId: "R1" });
    const created = mock.shareCreate({ providerId: mock.providerList().providers[0].id });
    expect(mock.shareRedeem(created.link).shareId).toBe(created.shareId);
    const again = mock.shareRedeem(created.link);
    expect(again.shareId).toBe(created.shareId);
    expect(mock.shareList().shares[0].activations).toBe(1);
    const other = seededMock({ redeemerPeerId: "R2" });
    // 同一条链接在出借方已被 R1 兑换（boundPeer=R1），R2 兑换 → bound-other
    other.mock.seedShare({
      token: parseLlmShareLink(created.link).token,
      models: PROVIDER_MODELS,
      boundPeer: "R1",
      activations: 1,
    });
    expect(() => other.mock.shareRedeem(created.link)).toThrow(/bound-other/);
  });

  it("revoke 后 redeem 拒绝 share-revoked 且级联删 allowlist 条目", () => {
    const { mock } = seededMock();
    const created = mock.shareCreate({ providerId: mock.providerList().providers[0].id });
    mock.shareRedeem(created.link);
    expect(mock.allowList().entries).toHaveLength(1);
    expect(mock.shareRevoke(created.shareId)).toEqual({ revoked: true });
    expect(mock.allowList().entries).toHaveLength(0);
    expect(() => mock.shareRedeem(created.link)).toThrow(/share-revoked/);
    expect(() => mock.shareRevoke(created.shareId)).toThrow(/already revoked/);
  });

  it("过期兑换拒绝 expired；未知 token 拒绝 invalid", () => {
    const { mock } = seededMock();
    mock.seedShare({ token: "f".repeat(32), models: ["gpt-4o"], expiresAtUnix: T0 - 1 });
    expect(() => mock.shareRedeem("dsh-llm-share://v1?peer=p&token=" + "f".repeat(32))).toThrow(/expired/);
    expect(() => mock.shareRedeem("dsh-llm-share://v1?peer=p&token=" + "e".repeat(32))).toThrow(/invalid/);
  });
});

describe("v13 serveStatus（§16.6 装配态：assembled:false 是常态非故障）", () => {
  it("缺省未装配；setServeStatus 驱动装配态与 lastError", () => {
    const { mock } = seededMock();
    expect(mock.serveStatus()).toEqual({ assembled: false, models: [] });
    mock.setServeStatus({ assembled: true, models: ["gpt-4o"], lastError: undefined });
    expect(mock.serveStatus()).toEqual({ assembled: true, models: ["gpt-4o"], lastError: undefined });
    mock.setServeStatus({ assembled: false, models: [], lastError: "offer missing" });
    expect(mock.serveStatus().lastError).toBe("offer missing");
  });
});
