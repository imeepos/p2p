// llm-share v13 命令面 mock 扩展（契约 §16.6）：provider / share / serve 三块。
// 从 mock-llm-share.ts 拆出守 300 行红线；语义对齐设计 §5.3/§5.4——share token 台账
// 只存 sha256（mock 为可兑换保留原文并注释说明）、兑换激活进 allowlist 回调、serve
// 装配态可经控制器驱动（assembled:false 是常态非故障）。
import type {
  LlmProviderSaveReq,
  LlmProviderView,
  LlmServeStatus,
  LlmShareCreateReq,
  LlmShareCreateResult,
  LlmShareEntry,
  LlmShareRedeemResult,
} from "./ipc-types";
import { buildLlmShareLink, parseLlmShareLink } from "./llm-share-link-model";

export interface LlmShareExtDeps {
  selfPeerId: () => string;
  offerModels: () => string[];
  nowSecs: () => number;
  redeemerPeerId: () => string;
  /** 兑换激活：写入 allowlist（source=share:<shareId>，模型集限定，到期=exp） */
  redeemToAllowlist: (peer: string, models: string[], source: string, expiresAtUnix: number) => void;
  /** 撤销级联：按 source 移除 allowlist 条目 */
  revokeSource: (source: string) => void;
}

export type MockSharePhase = "active" | "expired" | "revoked" | "exhausted" | null;

const DEFAULT_TTL_SECS = 24 * 3600;
const MAX_TTL_SECS = 7 * 24 * 3600;

function newTokenHex(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
}

export function createMockLlmShareExt(deps: LlmShareExtDeps) {
  const providers = new Map<string, LlmProviderView>();
  const shares: LlmShareEntry[] = [];
  const tokenToShareId = new Map<string, string>();
  let serve: LlmServeStatus = { assembled: false, models: [] };
  let forcedPhase: MockSharePhase = null;

  function providerView(input: LlmProviderSaveReq, id: string, createdAt: number): LlmProviderView {
    return {
      id,
      name: input.name,
      baseUrl: input.baseUrl,
      protocol: input.protocol,
      models: [...input.models],
      createdAt,
      apiKeyMasked: mask(input.apiKey ?? ""),
    };
  }

  function mask(apiKey: string): string {
    const trimmed = apiKey.trim();
    if (trimmed.length < 8) return "••••••••";
    return `${trimmed.slice(0, 3)}••••${trimmed.slice(-4)}`;
  }

  function requireText(value: string, field: string): string {
    const trimmed = value.trim();
    if (!trimmed) throw new Error(`llm-share mock: ${field} required`);
    return trimmed;
  }

  function shareStatusOf(entry: LlmShareEntry, nowSecs: number): LlmShareEntry["status"] {
    if (entry.revoked) return "revoked";
    if (entry.expiresAtUnix <= nowSecs) return "expired";
    if (entry.activations >= entry.maxActivations) return "exhausted";
    return "active";
  }

  const backend = {
    async llmShareProviderList(): Promise<{ providers: LlmProviderView[] }> {
      return { providers: [...providers.values()] };
    },

    async llmShareProviderSave(config: LlmProviderSaveReq): Promise<LlmProviderView> {
      const name = requireText(config.name, "name");
      const baseUrl = requireText(config.baseUrl, "baseUrl");
      if (config.models.length === 0) throw new Error("llm-share mock: models required (at least one)");
      const existing = config.id ? providers.get(config.id) : undefined;
      // apiKey 明文仅入参：缺省/空串 = 保留原密钥（列表只回掩码，编辑态无从回填明文）
      const apiKey = config.apiKey?.trim() ?? "";
      if (!existing && !apiKey) throw new Error("llm-share mock: apiKey required on create");
      const id = existing?.id ?? config.id ?? `pv-${Date.now().toString(36)}`;
      const createdAt = existing?.createdAt ?? Date.now();
      providers.set(id, providerView({ ...config, name, baseUrl }, id, createdAt));
      return providers.get(id)!;
    },

    async llmShareProviderRemove(providerId: string): Promise<{ removed: true }> {
      if (!providers.delete(providerId)) {
        // 不存在 = 显式报错非错误态（对齐 deny 语义）
        throw new Error(`llm-share mock: provider not found ${providerId} (explicit error, not a fault)`);
      }
      return { removed: true };
    },

    async llmShareShareCreate(req: LlmShareCreateReq): Promise<LlmShareCreateResult> {
      const provider = providers.get(req.providerId);
      if (!provider) throw new Error(`llm-share mock: provider not found ${req.providerId}`);
      const models = req.models && req.models.length > 0 ? [...req.models] : [...provider.models];
      if (models.length === 0) throw new Error("llm-share mock: share models required (non-empty)");
      const offerModels = deps.offerModels();
      for (const model of models) {
        if (!offerModels.includes(model)) {
          throw new Error(`llm-share mock: share model ${model} not in current offer`);
        }
      }
      const nowSecs = deps.nowSecs();
      const ttl = Math.min(req.expiresAt ? req.expiresAt - nowSecs : DEFAULT_TTL_SECS, MAX_TTL_SECS);
      const expiresAt = nowSecs + ttl;
      const token = newTokenHex();
      const shareId = `share-${Date.now().toString(36)}-${Math.floor(Math.random() * 1e6)}`;
      // 台账语义：只存 token sha256；mock 为可兑换保留原文映射（真实实现存哈希后原文即弃）
      tokenToShareId.set(token, shareId);
      shares.push({
        shareId,
        providerId: provider.id,
        models,
        maxActivations: 1,
        activations: 0,
        expiresAtUnix: expiresAt,
        revoked: false,
        note: req.note?.trim() ?? "",
        createdAt: nowSecs,
        boundPeer: null,
        status: "active",
      });
      const link = buildLlmShareLink({
        peer: deps.selfPeerId(),
        token,
        expUnix: expiresAt,
        sid: shareId,
        models,
      });
      return { link, shareId, expiresAt, models };
    },

    async llmShareShareList(): Promise<{ shares: LlmShareEntry[] }> {
      const nowSecs = deps.nowSecs();
      return {
        shares: shares.map((entry) => ({ ...entry, status: shareStatusOf(entry, nowSecs) })),
      };
    },

    async llmShareShareRevoke(shareId: string): Promise<{ revoked: true }> {
      const entry = shares.find((s) => s.shareId === shareId);
      if (!entry) throw new Error(`llm-share mock: share not found ${shareId} (explicit error, not a fault)`);
      if (entry.revoked) throw new Error(`llm-share mock: share ${shareId} already revoked`);
      entry.revoked = true;
      deps.revokeSource(`share:${shareId}`);
      return { revoked: true };
    },

    async llmShareShareRedeem(link: string): Promise<LlmShareRedeemResult> {
      if (forcedPhase === "revoked") throw new Error("redeem rejected: share-revoked");
      if (forcedPhase === "expired") throw new Error("redeem rejected: expired");
      if (forcedPhase === "exhausted") throw new Error("redeem rejected: exhausted");
      const parsed = parseLlmShareLink(link);
      const shareId = tokenToShareId.get(parsed.token);
      if (!shareId) throw new Error("redeem rejected: invalid");
      const entry = shares.find((s) => s.shareId === shareId);
      if (!entry) throw new Error("redeem rejected: invalid");
      const nowSecs = deps.nowSecs();
      if (entry.revoked) throw new Error("redeem rejected: share-revoked");
      if (entry.expiresAtUnix <= nowSecs) throw new Error("redeem rejected: expired");
      const redeemer = deps.redeemerPeerId();
      if (entry.boundPeer && entry.boundPeer !== redeemer) throw new Error("redeem rejected: bound-other");
      if (entry.activations >= entry.maxActivations) {
        // 同 peer 二次兑换 = 幂等成功（不重复计数，对齐 ACP §4）
        if (entry.boundPeer === redeemer) return redeemResult(entry, parsed.peer);
        throw new Error("redeem rejected: exhausted");
      }
      entry.activations += 1;
      entry.boundPeer = redeemer;
      deps.redeemToAllowlist(parsed.peer, entry.models, `share:${entry.shareId}`, entry.expiresAtUnix);
      return redeemResult(entry, parsed.peer);
    },

    async llmShareServeStatus(): Promise<LlmServeStatus> {
      return { ...serve, models: [...serve.models] };
    },
  };

  function redeemResult(entry: LlmShareEntry, peer: string): LlmShareRedeemResult {
    return {
      offer: { peer, models: [...entry.models], spare: {}, periodEnds: "" },
      shareId: entry.shareId,
      owner: deps.selfPeerId(),
    };
  }

  const controller = {
    setServeAssembled(assembled: boolean, info?: { providerId?: string; models?: string[]; lastError?: string }): void {
      serve = {
        assembled,
        providerId: info?.providerId,
        models: info?.models ?? [],
        lastError: info?.lastError,
      };
    },
    setSharePhase(phase: MockSharePhase): void {
      forcedPhase = phase;
    },
    reset(): void {
      providers.clear();
      shares.length = 0;
      tokenToShareId.clear();
      serve = { assembled: false, models: [] };
      forcedPhase = null;
    },
  };

  return { backend, controller };
}

export type MockLlmShareExtController = ReturnType<typeof createMockLlmShareExt>["controller"];