import type {
  LlmAllowEntry,
  LlmBorrowReport,
  LlmBorrowReq,
  LlmLedgerEntry,
  LlmReceiptVerifyReq,
  LlmReceiptVerifyResult,
  LlmRejectCode,
} from "./types";

// mock 借用结算与收据验签（ai-guide borrow / receipt verify 语义，§16.2-1/2/3）。
// 从 mock-backend 拆出以守 300 行红线；状态经引用注入，单向依赖。
export class LlmShareMockError extends Error {}

export interface ReceiptRecord {
  report: LlmBorrowReport;
  entry: LlmLedgerEntry | null;
}

export interface MockBorrowState {
  allowGet: (peerId: string) => LlmAllowEntry | undefined;
  ledger: LlmLedgerEntry[];
  receipts: Map<string, ReceiptRecord>;
  callLog: string[];
}

export interface MockBorrowConfig {
  localPeerId: string;
  now: () => number;
  breakStreamEveryBorrow: boolean;
  rejectCode?: LlmRejectCode | null;
  failVerifyReqIds?: string[];
  previewChars: number;
}

export const MOCK_LENDER_PUBKEY = "TkQj3nGvArK5vjNKPz7eSC9y3csxncb8o29KCTaxvZk";
const ESTIMATED_DISPUTE_SECS = 72 * 3600;
const CONFIRMED_DISPUTE_SECS = 24 * 3600;
const PERIOD = "2026-09";

function requireText(value: string, field: string): string {
  const trimmed = value.trim();
  if (!trimmed) throw new LlmShareMockError(`llm-share mock: ${field} required`);
  return trimmed;
}

export function settleBorrowCall(
  state: MockBorrowState,
  cfg: MockBorrowConfig,
  req: LlmBorrowReq,
): LlmBorrowReport {
  const normalized: LlmBorrowReq = {
    model: requireText(req.model, "model"),
    messages: requireText(req.messages, "messages"),
    targetPeer: requireText(req.targetPeer, "targetPeer"),
    maxTokens: req.maxTokens,
    reqId: req.reqId,
  };
  if (!Number.isInteger(normalized.maxTokens) || normalized.maxTokens <= 0) {
    throw new LlmShareMockError("borrow: maxTokens must be a positive integer");
  }
  const reqId = normalized.reqId ?? `req-${state.callLog.length + 1}`;
  state.callLog.push(reqId);
  const replay = state.receipts.get(reqId);
  if (replay) {
    // §16.2-3：重放返回原收据 appended=false，不双记账
    return { ...replay.report, receipt: { ...replay.report.receipt, appended: false } };
  }
  return settle({ ...normalized, reqId }, state, cfg);
}

function settle(req: LlmBorrowReq, state: MockBorrowState, cfg: MockBorrowConfig): LlmBorrowReport {
  const entry = state.allowGet(req.targetPeer);
  const code = resolveRejectCode(req, entry, cfg);
  if (code) {
    return {
      status: "rejected",
      code,
      message: "upstream untouched: borrow rejected structurally (default-deny or model not served)",
      sseCount: 0,
      receipt: { reqId: req.reqId ?? "", appended: false, estimated: false, disputeWindowSecs: 0 },
    };
  }
  const broken = cfg.breakStreamEveryBorrow;
  const input = broken ? 40 : 1234;
  const output = broken ? 0 : 567;
  const ledgerEntry: LlmLedgerEntry = {
    reqId: req.reqId ?? "",
    period: PERIOD,
    lender: req.targetPeer,
    borrower: cfg.localPeerId,
    model: req.model,
    input,
    output,
    tokens: input + output,
    estimated: broken,
    ts: cfg.now(),
  };
  state.ledger.push(ledgerEntry);
  const report: LlmBorrowReport = {
    status: broken ? "stream_broken" : "done",
    receipt: {
      reqId: ledgerEntry.reqId,
      appended: true,
      estimated: broken,
      disputeWindowSecs: broken ? ESTIMATED_DISPUTE_SECS : CONFIRMED_DISPUTE_SECS,
    },
    sseCount: broken ? 4 : 9,
    usage: { input, output },
    period: PERIOD,
    lender: req.targetPeer,
    message: buildPreview(broken, cfg.previewChars),
  };
  state.receipts.set(ledgerEntry.reqId, { report, entry: ledgerEntry });
  return report;
}

function resolveRejectCode(
  req: LlmBorrowReq,
  entry: LlmAllowEntry | undefined,
  cfg: MockBorrowConfig,
): LlmRejectCode | null {
  if (cfg.rejectCode) return cfg.rejectCode;
  if (!entry) return "not_allowlisted";
  if (entry.models.length > 0 && !entry.models.includes(req.model)) return "model_not_served";
  return null;
}

// §16.2-6：sse 原文只出计数，正文仅截断预览
function buildPreview(broken: boolean, limit: number): string {
  const body = broken
    ? '{"choices":[{"delta":{"content":"part"}}]} {"choices":[{"delta":{"content":"ial body before stream broke"}}]}'
    : '{"choices":[{"delta":{"content":"hello"}}]} {"choices":[{"delta":{"content":", streaming "}}]} {"choices":[{"delta":{"content":"body"}}]}';
  return body.length > limit ? `${body.slice(0, limit)}... (truncated)` : body;
}

export function verifyReceiptCall(
  state: MockBorrowState,
  cfg: MockBorrowConfig,
  req: LlmReceiptVerifyReq,
): LlmReceiptVerifyResult {
  const reqId = requireText(req.reqId, "reqId");
  const record = state.receipts.get(reqId);
  const entry = record?.entry;
  const base = {
    reqId,
    period: entry?.period,
    lender: entry?.lender,
    borrower: entry?.borrower,
    model: entry?.model,
    input: entry?.input,
    output: entry?.output,
    estimated: entry?.estimated,
    ts: entry?.ts,
  };
  if (cfg.failVerifyReqIds?.includes(reqId)) {
    return { ...base, verdict: "FAIL", reason: "verify failed: receipt signature invalid" };
  }
  if (req.lenderPubkey !== undefined && req.lenderPubkey !== MOCK_LENDER_PUBKEY) {
    return { ...base, verdict: "FAIL", reason: "pubkey-lender binding check failed (PeerId = sha256(pubkey))" };
  }
  if (!entry) {
    return { ...base, verdict: "FAIL", reason: `receipt not found: reqId=${reqId}` };
  }
  return { ...base, verdict: "PASS", reason: "signature verified" };
}
