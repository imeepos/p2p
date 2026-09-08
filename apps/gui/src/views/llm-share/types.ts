// 契约 gui-contract §16.1（v11 冻结）：llm-share GUI 面 DTO。
// 字段与 ai-guide --json 输出 camelCase 对齐；语义约束见 §16.2 一至七条。
// 本文件为视图层数据接缝类型，PR 轨语义方会签时以契约表逐字核对。
// 契约 §16.6 v13（llm-share-link 波）：双协议 provider + 分享链接 8 命令面 DTO。
// 与 IPC 接缝共用同一组定义（lib/llm-share-v13-types.ts），此处 re-export 防漂移。
import type {
  LlmProviderSaveReq,
  LlmProviderView,
  LlmServeStatus,
  LlmShareCreateReq,
  LlmShareCreateResult,
  LlmShareEntry,
  LlmShareRedeemResult,
} from "@/lib/llm-share-v13-types";
export type {
  LlmProviderProtocol,
  LlmProviderSaveReq,
  LlmProviderView,
  LlmServeStatus,
  LlmShareCreateReq,
  LlmShareCreateResult,
  LlmShareEntry,
  LlmShareRejectCode,
  LlmShareRedeemResult,
} from "@/lib/llm-share-v13-types";

export type LlmOfferStatus =
  | "live"
  | "expired"
  | "not_yet_valid"
  | "peer_mismatch"
  | "bad_signature";

export interface LlmOfferView {
  peer: string;
  models: string[];
  spare: Record<string, number>;
  periodEnds: string;
  maxPerReq?: Record<string, number>;
  rpm?: number;
  concurrency?: number;
  ttlSecs: number;
  retention?: string;
  issuedAt: number;
  expiresAt: number;
  remainingSecs: number;
  status: LlmOfferStatus;
}

export interface LlmOfferPublishReq {
  models: string[];
  spare: Record<string, number>;
  periodEnds: string;
  maxPerReq?: Record<string, number>;
  rpm?: number;
  concurrency?: number;
  ttlSecs?: number;
  retention?: string;
}

// ai-guide allow：--model 可重复，缺省 = 不限模型（空数组表达）。
export interface LlmAllowEntry {
  peerId: string;
  models: string[];
  note: string;
  grantedAt: string;
}

export interface LlmAllowlistView {
  entries: LlmAllowEntry[];
}

export interface LlmAllowReq {
  peerId: string;
  models?: string[];
  note?: string;
}

// §16.2-1：四值原样透出不本地化改写；rejected 是业务结果非命令 Err。
export type LlmRejectCode =
  | "not_allowlisted"
  | "model_not_served"
  | "freeze_insufficient"
  | "concurrency_exceeded";

export interface LlmBorrowReq {
  model: string;
  messages: string;
  maxTokens: number;
  targetPeer: string;
  reqId?: string;
}

export interface LlmBorrowReceipt {
  reqId: string;
  appended: boolean;
  estimated: boolean;
  disputeWindowSecs: number;
}

export interface LlmBorrowReport {
  status: "done" | "stream_broken" | "rejected";
  receipt: LlmBorrowReceipt;
  sseCount: number;
  usage?: { input: number; output: number };
  code?: LlmRejectCode;
  message?: string;
  period?: string;
  lender?: string;
}

export interface LlmLedgerFilter {
  lender?: string;
  borrower?: string;
  period?: string;
}

export interface LlmLedgerEntry {
  reqId: string;
  period: string;
  lender: string;
  borrower: string;
  model: string;
  input: number;
  output: number;
  tokens: number;
  estimated: boolean;
  ts: number;
}

export type LlmBalanceDirection = "lent" | "borrowed" | "flat";

// §16.1 balance：净差按 lender+period 切分，正负号 = 借贷方向；
// lentOut/borrowed/entries 为 ai-guide --json 行字段的超集，便于展示明细。
export interface LlmBalanceGroup {
  lender: string;
  period: string;
  lentOut: number;
  borrowed: number;
  netAmount: number;
  entries: number;
  direction: LlmBalanceDirection;
}

// §16.1 receipt verify：缺省本机身份仅出借方自验；借方场景须传出借方公钥。
export interface LlmReceiptVerifyReq {
  reqId: string;
  lenderPubkey?: string;
}

export interface LlmReceiptVerifyResult {
  verdict: "PASS" | "FAIL";
  reason: string;
  reqId: string;
  period?: string;
  lender?: string;
  borrower?: string;
  model?: string;
  input?: number;
  output?: number;
  estimated?: boolean;
  ts?: number;
}

export interface LlmShareBackend {
  offerPublish: (req: LlmOfferPublishReq) => Promise<LlmOfferView>;
  offerShow: () => Promise<LlmOfferView>;
  allowList: () => Promise<LlmAllowlistView>;
  allow: (req: LlmAllowReq) => Promise<LlmAllowlistView>;
  deny: (peerId: string) => Promise<LlmAllowlistView>;
  borrow: (req: LlmBorrowReq) => Promise<LlmBorrowReport>;
  ledgerList: (filter?: LlmLedgerFilter) => Promise<LlmLedgerEntry[]>;
  ledgerBalance: () => Promise<LlmBalanceGroup[]>;
  receiptVerify: (req: LlmReceiptVerifyReq) => Promise<LlmReceiptVerifyResult>;
  // 契约 §16.6 v13 加法（8 条）：双协议 provider + 分享链接命令面。
  providerList: () => Promise<{ providers: LlmProviderView[] }>;
  providerSave: (config: LlmProviderSaveReq) => Promise<LlmProviderView>;
  providerRemove: (providerId: string) => Promise<{ removed: true }>;
  shareCreate: (req: LlmShareCreateReq) => Promise<LlmShareCreateResult>;
  shareList: () => Promise<{ shares: LlmShareEntry[] }>;
  shareRevoke: (shareId: string) => Promise<{ revoked: true }>;
  shareRedeem: (link: string) => Promise<LlmShareRedeemResult>;
  serveStatus: () => Promise<LlmServeStatus>;
}
