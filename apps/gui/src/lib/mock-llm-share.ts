// llm-share 命令面 mock（契约 v11 §16 九命令 + v13 §16.6 八命令扩展）。mock 不得静态打进
// prod bundle（E9-Q0 T4）：由 mock-ipc 仅在 VITE_MOCK_IPC=1 时动态加载；失败路径显式抛错。
import { createMockLlmShareExt } from "./mock-llm-share-ext";
import { DONE_DISPUTE_SECS, recordBorrowReceipt, settleBorrowPhase } from "./mock-llm-share-borrow";
import type {
  LlmAllowEntry,
  LlmAllowlistView,
  LlmBalanceGroup,
  LlmBorrowRejectionCode,
  LlmBorrowReport,
  LlmBorrowRequest,
  LlmLedgerEntry,
  LlmLedgerFilter,
  LlmOfferPublishInput,
  LlmOfferStatus,
  LlmOfferView,
  LlmReceiptVerifyResult,
} from "./ipc-types";

const B58_RE = /^[1-9A-HJ-NP-Za-km-z]{43,44}$/;
const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;
const MOCK_OFFER_FILE = "/p2p-data/llm-share/offer.json";

export interface LlmShareMockDeps {
  selfPeerId: () => string;
  nowMs?: () => number;
}

interface LlmShareMockState {
  offerPhase: LlmOfferStatus | "none";
  rejection: LlmBorrowRejectionCode | null;
  streamBroken: boolean;
  offer: LlmOfferView | null;
  allowlist: Map<string, LlmAllowEntry>;
  ledger: LlmLedgerEntry[];
  receipts: Map<string, LlmReceiptVerifyResult>;
}

function initialState(): LlmShareMockState {
  return {
    offerPhase: "none",
    rejection: null,
    streamBroken: false,
    offer: null,
    allowlist: new Map(),
    ledger: [],
    receipts: new Map(),
  };
}

function requirePeerId(peerId: string): void {
  // 对齐 CLI 退出码语义：PeerId 非法（非 base58 或解码后非 32 字节）→ 显式报错。
  if (!B58_RE.test(peerId)) throw new Error(`PeerId 非法（须为 base58 的 32 字节编码）: "${peerId}"`);
}

// §16.2.5：expired/not_yet_valid=常态中性；peer_mismatch/bad_signature=警示态。
function offerStatusFor(phase: LlmOfferStatus, remainingSecs: number): LlmOfferStatus {
  if (phase === "live") return remainingSecs > 0 ? "live" : "expired";
  return phase;
}

export function createMockLlmShare(deps: LlmShareMockDeps) {
  const state = initialState();
  const now = deps.nowMs ?? (() => Date.now());

  function allowEntries(): LlmAllowEntry[] {
    return [...state.allowlist.values()]
      .sort((a, b) => (a.peerId < b.peerId ? -1 : 1))
      .map((e) => ({ ...e, models: e.models ? [...e.models] : null }));
  }

  function allowView(peerId: string, ok: boolean, created: boolean, message: string): LlmAllowlistView {
    return { entries: allowEntries(), lastOp: { op: "allow", peerId, ok, created, message } };
  }

  function denyView(peerId: string, ok: boolean, message: string): LlmAllowlistView {
    return { entries: allowEntries(), lastOp: { op: "deny", peerId, ok, created: false, message } };
  }

  const baseBackend = {
    async llmShareOfferPublish(offer: LlmOfferPublishInput): Promise<LlmOfferView> {
      if (!offer.models.length) throw new Error("models 至少一个（必填集）");
      if (new Set(offer.models).size !== offer.models.length) throw new Error("models 存在重复声明");
      for (const model of offer.models) {
        const spare = offer.spare[model];
        if (spare === undefined) throw new Error(`spare 未覆盖模型 ${model}`);
        if (!(spare > 0)) throw new Error(`spare[${model}] 须为正数`);
      }
      for (const key of Object.keys(offer.spare)) {
        if (!offer.models.includes(key)) throw new Error(`spare 引用未声明模型 ${key}`);
      }
      if (!DATE_RE.test(offer.periodEnds)) {
        throw new Error(`periodEnds 须为 YYYY-MM-DD 日期，实得 "${offer.periodEnds}"`);
      }
      if (offer.rpm !== undefined && offer.rpm <= 0) throw new Error("rpm 须为正数");
      if (offer.concurrency !== undefined && offer.concurrency <= 0) throw new Error("concurrency 须为正数");
      if (offer.ttlSecs !== undefined && offer.ttlSecs <= 0) throw new Error("ttlSecs 须为正数");
      const issuedAt = Math.floor(now() / 1000);
      const ttl = offer.ttlSecs ?? 3600;
      state.offer = {
        peer: deps.selfPeerId(),
        models: [...offer.models],
        spare: { ...offer.spare },
        periodEnds: offer.periodEnds,
        maxPerReq: offer.maxPerReq ? { ...offer.maxPerReq } : null,
        rateLimit: { rpm: offer.rpm ?? 10, concurrency: offer.concurrency ?? 2 },
        ttlSecs: ttl,
        retention: offer.retention ?? "none",
        issuedAt,
        expiresAt: issuedAt + ttl,
        remainingSecs: ttl,
        status: "live",
        file: MOCK_OFFER_FILE,
      };
      state.offerPhase = "live";
      return { ...state.offer };
    },

    async llmShareOfferShow(): Promise<LlmOfferView> {
      if (!state.offer || state.offerPhase === "none") {
        throw new Error("尚未发布出借声明（offer），请先发布（CLI 同语义：从未发布显式报错）");
      }
      const remainingSecs = state.offer.expiresAt - Math.floor(now() / 1000);
      return {
        ...state.offer,
        remainingSecs,
        status: offerStatusFor(state.offerPhase, remainingSecs),
      };
    },

    async llmShareAllowList(): Promise<{ entries: LlmAllowEntry[] }> {
      return { entries: allowEntries() };
    },

    async llmShareAllow(peerId: string, models?: string[], note?: string): Promise<LlmAllowlistView> {
      requirePeerId(peerId);
      if (models?.some((m) => !m)) throw new Error("模型名为空");
      const created = !state.allowlist.has(peerId);
      state.allowlist.set(peerId, {
        peerId,
        models: models ? [...models] : null, // 缺省 = 不限模型（CLI 原话）
        note: note ?? null,
        grantedAt: new Date(now()).toISOString(),
      });
      return allowView(peerId, true, created, created ? "已加入 allowlist（新建条目）" : "已更新条目并刷新 grantedAt");
    },

    async llmShareDeny(peerId: string): Promise<LlmAllowlistView> {
      requirePeerId(peerId);
      const existed = state.allowlist.delete(peerId);
      return existed
        ? denyView(peerId, true, "已移出 allowlist（回到默认拒绝）")
        : denyView(peerId, false, "allowlist 无该借方条目（默认拒绝语义，非故障）");
    },

    async llmShareBorrow(req: LlmBorrowRequest): Promise<LlmBorrowReport> {
      if (!req.targetPeer) throw new Error("缺少出借方 targetPeer（无缺省路径，IPC 层显式报错）");
      requirePeerId(req.targetPeer);
      if (!req.maxTokens || req.maxTokens <= 0) throw new Error("maxTokens 必填且须为正数（真实成本动作，显式上限）");
      if (!req.model) throw new Error("model 必填");
      if (!req.messages.length) throw new Error("messages 不能为空");
      const reqId = req.reqId ?? crypto.randomUUID();
      const replay = state.receipts.get(reqId);
      // §16.2.3：req_id 幂等——重试复用同 reqId，返回原收据且 appended=false 不双记。
      if (replay) return {
        status: "done",
        receipt: { reqId, appended: false, estimated: replay.estimated, disputeWindowSecs: DONE_DISPUTE_SECS },
        sseCount: 0,
        usage: { input: replay.input, output: replay.output },
        message: "req_id 重放：返回原收据，不双记账",
      };
      const report = settleBorrowPhase(state, reqId);
      if (report.receipt.appended) {
        recordBorrowReceipt(state, req, reqId, report, Math.floor(now() / 1000), deps.selfPeerId());
      }
      return report;
    },

    async llmShareLedgerList(filter?: LlmLedgerFilter): Promise<LlmLedgerEntry[]> {
      return state.ledger
        .filter((e) => !filter?.lender || e.lender === filter.lender)
        .filter((e) => !filter?.borrower || e.borrower === filter.borrower)
        .filter((e) => !filter?.period || e.period === filter.period)
        .map((e) => ({ ...e }));
    },

    async llmShareLedgerBalance(): Promise<LlmBalanceGroup[]> {
      const self = deps.selfPeerId();
      const groups = new Map<string, { lender: string; period: string; lentOut: number; borrowed: number }>();
      for (const e of state.ledger) {
        if (e.lender !== self && e.borrower !== self) continue;
        const key = e.lender + "@" + e.period;
        const g = groups.get(key) ?? { lender: e.lender, period: e.period, lentOut: 0, borrowed: 0 };
        if (e.lender === self) g.lentOut += e.tokens;
        if (e.borrower === self) g.borrowed += e.tokens;
        groups.set(key, g);
      }
      return [...groups.values()]
        .sort((a, b) => (a.lender + a.period < b.lender + b.period ? -1 : 1))
        .map((g) => {
          const netAmount = g.lentOut - g.borrowed;
          return {
            lender: g.lender,
            period: g.period,
            netAmount,
            direction: netAmount >= 0 ? ("lent_out" as const) : ("borrowed" as const),
          };
        });
    },

    async llmShareReceiptVerify(reqId: string, lenderPubkey?: string): Promise<LlmReceiptVerifyResult> {
      const payload = state.receipts.get(reqId);
      if (!payload) throw new Error(`收据不存在或已损坏（req_id 未知）: ${reqId}`);
      if (lenderPubkey && lenderPubkey !== payload.lender) {
        return { ...payload, verdict: "FAIL" as const, reason: "公钥与 lender PeerId 不绑定" };
      }
      return { ...payload };
    },
  };

  const baseController = {
    setOfferPhase(phase: LlmOfferStatus | "none"): void {
      state.offerPhase = phase;
    },
    setRejection(code: LlmBorrowRejectionCode | null): void {
      state.rejection = code;
    },
    setStreamBroken(enabled: boolean): void {
      state.streamBroken = enabled;
    },
    reset(): void {
      Object.assign(state, initialState());
    },
  };

  // v13 扩展（provider/share/serve）：兑接 offer 快照与 allowlist 写入回调，
  // 控制器相位合并（reset 双清），backend 展开合并后由 mock-ipc 整体接线。
  const ext = createMockLlmShareExt({
    selfPeerId: deps.selfPeerId,
    offerModels: () => state.offer?.models ?? [],
    nowSecs: () => Math.floor(now() / 1000),
    redeemerPeerId: deps.selfPeerId,
    redeemToAllowlist: (peer, models, source) => {
      state.allowlist.set(peer, {
        peerId: peer,
        models: [...models],
        note: source,
        grantedAt: new Date(now()).toISOString(),
      });
    },
    revokeSource: (source) => {
      for (const [peerId, entry] of state.allowlist) {
        if (entry.note === source) state.allowlist.delete(peerId);
      }
    },
  });
  const controller = {
    ...baseController,
    ...ext.controller,
    reset: () => {
      baseController.reset();
      ext.controller.reset();
    },
  };

  return { backend: { ...baseBackend, ...ext.backend }, controller };
}

export type MockLlmShareController = ReturnType<typeof createMockLlmShare>["controller"];
