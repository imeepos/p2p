// mock-share：provider/share/serve 三块内存状态（契约 §16.6 v13）。从 mock-backend.ts
// 拆出守 300 行红线；share token 台账为可兑换保留原文映射（真实实现只存 sha256），
// 兑换激活经注入回调写 allowlist（source=share:<shareId>），revoke 按 source 级联删。
import { buildLlmShareLink, parseLlmShareLink } from "@/lib/llm-share-link-model";

import { LlmShareMockError } from "./mock-borrow";
import type {
  LlmAllowEntry,
  LlmOfferView,
  LlmProviderSaveReq,
  LlmProviderView,
  LlmServeStatus,
  LlmShareCreateReq,
  LlmShareCreateResult,
  LlmShareEntry,
  LlmShareRedeemResult,
} from "./types";

export interface MockShareDeps {
  now: () => number;
  selfPeerId: () => string;
  /** 当前 offer 快照（shareCreate 校验 models ⊆ offer.models；redeem 应答内嵌快照） */
  offerSnapshot: () => LlmOfferView | null;
  /** 兑换者 PeerId（真实语义=握手认证入站 peer；mock 注入可测 bound-other） */
  redeemerPeerId: () => string;
  allowlistSet: (entry: LlmAllowEntry) => void;
  allowlistRemoveBySource: (source: string) => void;
}

const DEFAULT_TTL_SECS = 24 * 3600;
const MAX_TTL_SECS = 7 * 24 * 3600;

function newTokenHex(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
}

function mask(apiKey: string): string {
  const trimmed = apiKey.trim();
  if (trimmed.length < 8) return "••••••••";
  return `${trimmed.slice(0, 3)}••••${trimmed.slice(-4)}`;
}

function requireText(value: string, field: string): string {
  const trimmed = value.trim();
  if (!trimmed) throw new LlmShareMockError(`llm-share mock: ${field} required`);
  return trimmed;
}

export class MockShareStore {
  private readonly deps: MockShareDeps;
  private readonly providers = new Map<string, LlmProviderView>();
  private readonly shares: LlmShareEntry[] = [];
  private readonly tokenToShareId = new Map<string, string>();
  private serve: LlmServeStatus = { assembled: false, models: [] };

  constructor(deps: MockShareDeps) {
    this.deps = deps;
  }

  providerList(): { providers: LlmProviderView[] } {
    return { providers: [...this.providers.values()] };
  }

  providerSave(config: LlmProviderSaveReq): LlmProviderView {
    const name = requireText(config.name, "name");
    const baseUrl = requireText(config.baseUrl, "baseUrl");
    if (config.models.length === 0) throw new LlmShareMockError("providerSave: models required (at least one)");
    const existing = config.id ? this.providers.get(config.id) : undefined;
    const apiKey = config.apiKey?.trim() ?? "";
    if (!existing && !apiKey) throw new LlmShareMockError("providerSave: apiKey required on create");
    const id = existing?.id ?? config.id ?? newProviderId();
    const createdAt = existing?.createdAt ?? this.deps.now();
    // 更新留空 = 保留原密钥：直接沿用已有掩码，绝不二次掩码（会叠出更长假掩码）
    const apiKeyMasked = apiKey ? mask(apiKey) : (existing?.apiKeyMasked ?? mask(""));
    const view: LlmProviderView = {
      id,
      name,
      baseUrl,
      protocol: config.protocol,
      models: [...config.models],
      createdAt,
      apiKeyMasked,
    };
    this.providers.set(id, view);
    return view;
  }

  providerRemove(providerId: string): { removed: true } {
    // 不存在 = 显式报错非错误态（对齐 deny 语义）
    if (!this.providers.delete(providerId)) {
      throw new LlmShareMockError(`providerRemove: not found ${providerId} (explicit error, not a fault)`);
    }
    return { removed: true };
  }

  shareCreate(req: LlmShareCreateReq): LlmShareCreateResult {
    const provider = this.providers.get(req.providerId);
    if (!provider) throw new LlmShareMockError(`shareCreate: provider not found ${req.providerId}`);
    // models 缺省（undefined）= provider 全模型；显式空数组 = 必填错误，不回落
    const models = req.models ? [...req.models] : [...provider.models];
    if (models.length === 0) throw new LlmShareMockError("shareCreate: models required (non-empty)");
    const offer = this.deps.offerSnapshot();
    const offerModels = offer?.models ?? [];
    for (const model of models) {
      if (!offerModels.includes(model)) {
        throw new LlmShareMockError(`shareCreate: model ${model} not in current offer`);
      }
    }
    const nowSecs = this.deps.now();
    const ttl = Math.min(req.expiresAt ? req.expiresAt - nowSecs : DEFAULT_TTL_SECS, MAX_TTL_SECS);
    const expiresAt = nowSecs + ttl;
    const token = newTokenHex();
    const shareId = `share-${Date.now().toString(36)}-${Math.floor(Math.random() * 1e6)}`;
    this.tokenToShareId.set(token, shareId);
    this.shares.push({
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
      peer: this.deps.selfPeerId(),
      token,
      expUnix: expiresAt,
      sid: shareId,
      models,
    });
    return { link, shareId, expiresAt, models };
  }

  shareList(): { shares: LlmShareEntry[] } {
    const nowSecs = this.deps.now();
    return {
      shares: this.shares.map((entry) => ({ ...entry, status: this.statusOf(entry, nowSecs) })),
    };
  }

  shareRevoke(shareId: string): { revoked: true } {
    const entry = this.shares.find((s) => s.shareId === shareId);
    if (!entry) throw new LlmShareMockError(`shareRevoke: not found ${shareId} (explicit error, not a fault)`);
    if (entry.revoked) throw new LlmShareMockError(`shareRevoke: ${shareId} already revoked`);
    entry.revoked = true;
    this.deps.allowlistRemoveBySource(`share:${shareId}`);
    return { revoked: true };
  }

  shareRedeem(link: string): LlmShareRedeemResult {
    const parsed = parseLlmShareLink(link);
    const entry = this.shares.find((s) => s.shareId === this.tokenToShareId.get(parsed.token));
    if (!entry) throw new LlmShareMockError("redeem rejected: invalid");
    const nowSecs = this.deps.now();
    if (entry.revoked) throw new LlmShareMockError("redeem rejected: share-revoked");
    if (entry.expiresAtUnix <= nowSecs) throw new LlmShareMockError("redeem rejected: expired");
    const redeemer = this.deps.redeemerPeerId();
    if (entry.boundPeer && entry.boundPeer !== redeemer) {
      throw new LlmShareMockError("redeem rejected: bound-other");
    }
    if (entry.activations >= entry.maxActivations) {
      // 同 peer 二次兑换 = 幂等成功（不重复计数，对齐 ACP §4）
      if (entry.boundPeer === redeemer) return this.redeemResult(entry, parsed.peer);
      throw new LlmShareMockError("redeem rejected: exhausted");
    }
    entry.activations += 1;
    entry.boundPeer = redeemer;
    this.deps.allowlistSet({
      peerId: parsed.peer,
      models: [...entry.models],
      note: `share:${entry.shareId}`,
      grantedAt: new Date(nowSecs * 1000).toISOString(),
    });
    return this.redeemResult(entry, parsed.peer);
  }

  serveStatus(): LlmServeStatus {
    return { ...this.serve, models: [...this.serve.models] };
  }

  setServeStatus(status: LlmServeStatus): void {
    this.serve = { ...status, models: [...status.models] };
  }

  /** 测试种子：直接登记一条分享（绕过 token 生成，绑定固定 token） */
  seedShare(share: Partial<LlmShareEntry> & { token: string }): LlmShareEntry {
    const nowSecs = this.deps.now();
    const entry: LlmShareEntry = {
      shareId: share.shareId ?? `share-seed-${this.shares.length}`,
      providerId: share.providerId ?? "pv-seed",
      models: share.models ?? [],
      maxActivations: share.maxActivations ?? 1,
      activations: share.activations ?? 0,
      expiresAtUnix: share.expiresAtUnix ?? nowSecs + DEFAULT_TTL_SECS,
      revoked: share.revoked ?? false,
      note: share.note ?? "",
      createdAt: nowSecs,
      boundPeer: share.boundPeer ?? null,
      status: "active",
    };
    this.tokenToShareId.set(share.token, entry.shareId);
    this.shares.push(entry);
    return entry;
  }

  private statusOf(entry: LlmShareEntry, nowSecs: number): LlmShareEntry["status"] {
    if (entry.revoked) return "revoked";
    if (entry.expiresAtUnix <= nowSecs) return "expired";
    if (entry.activations >= entry.maxActivations) return "exhausted";
    return "active";
  }

  private redeemResult(entry: LlmShareEntry, peer: string): LlmShareRedeemResult {
    const offer = this.deps.offerSnapshot();
    return {
      offer: {
        peer,
        models: [...entry.models],
        spare: offer ? { ...offer.spare } : {},
        periodEnds: offer?.periodEnds ?? "",
      },
      shareId: entry.shareId,
      owner: this.deps.selfPeerId(),
    };
  }
}

function newProviderId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return "pv-" + Math.random().toString(36).slice(2) + Date.now().toString(36);
}
