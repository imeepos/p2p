import type {
  LlmAllowEntry,
  LlmAllowlistView,
  LlmBalanceGroup,
  LlmBorrowReport,
  LlmBorrowReq,
  LlmLedgerEntry,
  LlmLedgerFilter,
  LlmOfferPublishReq,
  LlmOfferStatus,
  LlmOfferView,
  LlmProviderSaveReq,
  LlmProviderView,
  LlmReceiptVerifyReq,
  LlmReceiptVerifyResult,
  LlmRejectCode,
  LlmServeStatus,
  LlmShareBackend,
  LlmShareCreateReq,
  LlmShareCreateResult,
  LlmShareEntry,
  LlmShareRedeemResult,
} from "./types";
import { MockShareStore } from "./mock-share";

import {
  LlmShareMockError,
  settleBorrowCall,
  verifyReceiptCall,
  MOCK_LENDER_PUBKEY,
  type MockBorrowConfig,
  type MockBorrowState,
  type ReceiptRecord,
} from "./mock-borrow";

// mock 后端壳：allowlist/offer/ledger 三块内存状态 + 委托 mock-borrow 结算。
// 仅 VITE_MOCK_IPC=1 动态装载（no-mock-in-build 门禁保证不进产物）；测试经
// makeLlmShareMockPair 拿实例断言内部调用序。语义细节见 mock-borrow.ts。
export interface LlmShareMockOptions {
  now?: () => number;
  localPeerId?: string;
  forceOfferStatus?: LlmOfferStatus;
  breakStreamEveryBorrow?: boolean;
  rejectCode?: LlmRejectCode | null;
  failVerifyReqIds?: string[];
  previewChars?: number;
  /** 兑换者 PeerId（真实语义=握手认证入站 peer；缺省固定 mock 借方） */
  redeemerPeerId?: string;
}

// 固定 mock 借方：redeem 场景下作为兑换者写入 allowlist 的 PeerId
const MOCK_REDEEMER_PEER = "3wqibX4fA7GvGC1gpGZnEfBEYZPcx4DFnGEBi3yYb2Jv";

export { LlmShareMockError, MOCK_LENDER_PUBKEY };
export const MOCK_LOCAL_PEER = "7V8SRkBS6XLhS731XBcYbpjGBDctApRsbo49w2xhJGSk";

function requireNonEmpty(value: string, field: string): string {
  const trimmed = value.trim();
  if (!trimmed) throw new LlmShareMockError(`llm-share mock: ${field} required`);
  return trimmed;
}

export class LlmShareMock {
  private readonly opts: LlmShareMockOptions;
  private readonly allowEntries = new Map<string, LlmAllowEntry>();
  private readonly ledger: LlmLedgerEntry[] = [];
  private readonly receipts = new Map<string, ReceiptRecord>();
  private readonly callLog: string[] = [];
  private offer: LlmOfferView | null = null;
  // v13 扩展（provider/share/serve）：状态独立存储，allowlist 经注入回调联动
  private readonly shareStore: MockShareStore;

  constructor(opts: LlmShareMockOptions = {}) {
    this.opts = opts;
    this.shareStore = new MockShareStore({
      now: () => this.now(),
      selfPeerId: () => this.localPeerId,
      offerSnapshot: () => this.offer,
      redeemerPeerId: () => this.opts.redeemerPeerId ?? MOCK_REDEEMER_PEER,
      allowlistSet: (entry) => {
        this.allowEntries.set(entry.peerId, entry);
      },
      allowlistRemoveBySource: (source) => {
        for (const [peerId, entry] of this.allowEntries) {
          if (entry.note === source) this.allowEntries.delete(peerId);
        }
      },
    });
  }

  private now(): number {
    return (this.opts.now ?? (() => Math.floor(Date.now() / 1000)))();
  }

  get localPeerId(): string {
    return this.opts.localPeerId ?? MOCK_LOCAL_PEER;
  }

  get borrowReqIds(): string[] {
    return [...this.callLog];
  }

  private get borrowState(): MockBorrowState {
    return {
      allowGet: (peerId) => this.allowEntries.get(peerId),
      ledger: this.ledger,
      receipts: this.receipts,
      callLog: this.callLog,
    };
  }

  private get borrowConfig(): MockBorrowConfig {
    return {
      localPeerId: this.localPeerId,
      now: () => this.now(),
      breakStreamEveryBorrow: this.opts.breakStreamEveryBorrow === true,
      rejectCode: this.opts.rejectCode ?? null,
      failVerifyReqIds: this.opts.failVerifyReqIds,
      previewChars: this.opts.previewChars ?? 96,
    };
  }

  seedLedgerEntry(entry: LlmLedgerEntry): void {
    this.ledger.push(entry);
  }

  allow(req: { peerId: string; models?: string[]; note?: string }): LlmAllowlistView {
    const peerId = requireNonEmpty(req.peerId, "peerId");
    const existing = this.allowEntries.get(peerId);
    const entry: LlmAllowEntry = {
      peerId,
      models: req.models ?? existing?.models ?? [],
      note: req.note ?? existing?.note ?? "",
      grantedAt: new Date(this.now() * 1000).toISOString(),
    };
    this.allowEntries.set(peerId, entry);
    return { entries: this.listEntries() };
  }

  allowList(): LlmAllowlistView {
    return { entries: this.listEntries() };
  }

  deny(peerId: string): LlmAllowlistView {
    const key = requireNonEmpty(peerId, "peerId");
    // ai-guide deny：条目不存在 → 明确报错不静默（默认拒绝语义，非故障）
    if (!this.allowEntries.delete(key)) {
      throw new LlmShareMockError(`deny: entry not found peer=${key} (default-deny semantics, not a fault)`);
    }
    return { entries: this.listEntries() };
  }

  offerPublish(req: LlmOfferPublishReq): LlmOfferView {
    this.validatePublish(req);
    const issuedAt = this.now();
    const ttl = req.ttlSecs ?? 3600;
    const view: LlmOfferView = {
      peer: this.localPeerId,
      models: [...req.models],
      spare: { ...req.spare },
      periodEnds: req.periodEnds,
      maxPerReq: req.maxPerReq ? { ...req.maxPerReq } : undefined,
      rpm: req.rpm,
      concurrency: req.concurrency,
      ttlSecs: ttl,
      retention: req.retention ?? "none",
      issuedAt,
      expiresAt: issuedAt + ttl,
      remainingSecs: ttl,
      status: this.opts.forceOfferStatus ?? "live",
    };
    this.offer = view;
    return view;
  }

  private validatePublish(req: LlmOfferPublishReq): void {
    // ai-guide publish 退出 1 清单：models 空 / spare 未覆盖或为 0 / 引用
    // 未声明模型 / 日期非法。
    if (req.models.length === 0) throw new LlmShareMockError("publish: --model is empty");
    for (const model of req.models) {
      const spare = req.spare[model];
      if (spare === undefined) throw new LlmShareMockError(`publish: spare missing for ${model}`);
      if (!(spare > 0)) throw new LlmShareMockError(`publish: spare must be positive ${model}=${spare}`);
    }
    for (const key of Object.keys(req.spare)) {
      if (!req.models.includes(key)) throw new LlmShareMockError(`publish: spare references undeclared model ${key}`);
    }
    if (!/^\d{4}-\d{2}-\d{2}$/.test(req.periodEnds)) {
      throw new LlmShareMockError(`publish: invalid date ${req.periodEnds}`);
    }
  }

  offerShow(): LlmOfferView {
    if (!this.offer) throw new LlmShareMockError("never published: run offer publish first");
    return this.computeOfferStatus(this.offer);
  }

  private computeOfferStatus(offer: LlmOfferView): LlmOfferView {
    if (this.opts.forceOfferStatus) return { ...offer, status: this.opts.forceOfferStatus };
    const now = this.now();
    if (now >= offer.expiresAt) return { ...offer, status: "expired", remainingSecs: 0 };
    if (now < offer.issuedAt) return { ...offer, status: "not_yet_valid", remainingSecs: 0 };
    return { ...offer, status: "live", remainingSecs: offer.expiresAt - now };
  }

  borrow(req: LlmBorrowReq): LlmBorrowReport {
    return settleBorrowCall(this.borrowState, this.borrowConfig, req);
  }

  ledgerList(filter: LlmLedgerFilter = {}): LlmLedgerEntry[] {
    return this.ledger.filter(
      (e) =>
        (filter.lender === undefined || e.lender === filter.lender) &&
        (filter.borrower === undefined || e.borrower === filter.borrower) &&
        (filter.period === undefined || e.period === filter.period),
    );
  }

  ledgerBalance(): LlmBalanceGroup[] {
    const groups = new Map<string, LlmBalanceGroup>();
    for (const e of this.ledger) {
      if (e.lender !== this.localPeerId && e.borrower !== this.localPeerId) continue;
      const key = `${e.lender}#${e.period}`;
      const row = groups.get(key) ?? {
        lender: e.lender,
        period: e.period,
        lentOut: 0,
        borrowed: 0,
        netAmount: 0,
        entries: 0,
        direction: "flat" as const,
      };
      if (e.lender === this.localPeerId) row.lentOut += e.tokens;
      if (e.borrower === this.localPeerId) row.borrowed += e.tokens;
      row.netAmount = row.lentOut - row.borrowed;
      row.entries += 1;
      row.direction = row.netAmount > 0 ? "lent" : row.netAmount < 0 ? "borrowed" : "flat";
      groups.set(key, row);
    }
    return [...groups.values()];
  }

  receiptVerify(req: LlmReceiptVerifyReq): LlmReceiptVerifyResult {
    return verifyReceiptCall(this.borrowState, this.borrowConfig, req);
  }

  // ---- 契约 §16.6 v13：provider/share/serve（状态与语义见 mock-share.ts）----

  providerList(): { providers: LlmProviderView[] } {
    return this.shareStore.providerList();
  }

  providerSave(config: LlmProviderSaveReq): LlmProviderView {
    return this.shareStore.providerSave(config);
  }

  providerRemove(providerId: string): { removed: true } {
    return this.shareStore.providerRemove(providerId);
  }

  shareCreate(req: LlmShareCreateReq): LlmShareCreateResult {
    return this.shareStore.shareCreate(req);
  }

  shareList(): { shares: LlmShareEntry[] } {
    return this.shareStore.shareList();
  }

  shareRevoke(shareId: string): { revoked: true } {
    return this.shareStore.shareRevoke(shareId);
  }

  shareRedeem(link: string): LlmShareRedeemResult {
    return this.shareStore.shareRedeem(link);
  }

  serveStatus(): LlmServeStatus {
    return this.shareStore.serveStatus();
  }

  /** 测试种子：直接登记一条分享（绑定固定 token，redeem 可测） */
  seedShare(share: Partial<LlmShareEntry> & { token: string }): LlmShareEntry {
    return this.shareStore.seedShare(share);
  }

  setServeStatus(status: LlmServeStatus): void {
    this.shareStore.setServeStatus(status);
  }

  private listEntries(): LlmAllowEntry[] {
    return [...this.allowEntries.values()].sort((a, b) => a.peerId.localeCompare(b.peerId));
  }
}

function wireBackend(mock: LlmShareMock): LlmShareBackend {
  return {
    offerPublish: (req) => Promise.resolve(mock.offerPublish(req)),
    offerShow: () => Promise.resolve(mock.offerShow()),
    allowList: () => Promise.resolve(mock.allowList()),
    allow: (req) => Promise.resolve(mock.allow(req)),
    deny: (peerId) => Promise.resolve(mock.deny(peerId)),
    borrow: (req) => Promise.resolve(mock.borrow(req)),
    ledgerList: (filter) => Promise.resolve(mock.ledgerList(filter)),
    ledgerBalance: () => Promise.resolve(mock.ledgerBalance()),
    receiptVerify: (req) => Promise.resolve(mock.receiptVerify(req)),
    providerList: () => Promise.resolve(mock.providerList()),
    providerSave: (config) => Promise.resolve(mock.providerSave(config)),
    providerRemove: (providerId) => Promise.resolve(mock.providerRemove(providerId)),
    shareCreate: (req) => Promise.resolve(mock.shareCreate(req)),
    shareList: () => Promise.resolve(mock.shareList()),
    shareRevoke: (shareId) => Promise.resolve(mock.shareRevoke(shareId)),
    shareRedeem: (link) => Promise.resolve(mock.shareRedeem(link)),
    serveStatus: () => Promise.resolve(mock.serveStatus()),
  };
}

export function makeLlmShareMockBackend(opts: LlmShareMockOptions = {}): LlmShareBackend {
  return wireBackend(new LlmShareMock(opts));
}

export function makeLlmShareMockPair(
  opts: LlmShareMockOptions = {},
): { mock: LlmShareMock; backend: LlmShareBackend } {
  const mock = new LlmShareMock(opts);
  return { mock, backend: wireBackend(mock) };
}
