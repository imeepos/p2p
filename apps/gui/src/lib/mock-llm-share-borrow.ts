// llm-share borrow 结算拆分模块（契约 §16.2-1/2/3）：从 mock-llm-share.ts 拆出守
// 300 行红线。rejection 优先 → stream_broken 其次 → done 默认；req_id 幂等重放由
// 调用方（llmShareBorrow）依据 receipts 快照短路，本模块只做相位结算与收据入账。
import type {
  LlmBorrowRejectionCode,
  LlmBorrowReport,
  LlmBorrowRequest,
  LlmLedgerEntry,
  LlmReceiptVerifyResult,
} from "./ipc-types";

const STREAM_BROKEN_DISPUTE_SECS = 72 * 3600; // §16.2.2：估算账单 72h 争议窗
export const DONE_DISPUTE_SECS = 24 * 3600;

export interface MockBorrowSettleState {
  rejection: LlmBorrowRejectionCode | null;
  streamBroken: boolean;
  ledger: LlmLedgerEntry[];
  receipts: Map<string, LlmReceiptVerifyResult>;
}

export function settleBorrowPhase(
  state: MockBorrowSettleState,
  reqId: string,
): LlmBorrowReport {
  if (state.rejection) {
    return {
      status: "rejected",
      receipt: { reqId, appended: false, estimated: false, disputeWindowSecs: 0 },
      sseCount: 0,
      usage: null,
      code: state.rejection,
      message: `rejected: ${state.rejection}（上游零调用、流水零产生）`,
    };
  }
  if (state.streamBroken) {
    return {
      status: "stream_broken",
      receipt: { reqId, appended: true, estimated: true, disputeWindowSecs: STREAM_BROKEN_DISPUTE_SECS },
      sseCount: 5,
      usage: { input: 1234, output: 567 },
      message: "流式响应中断：入账为估算账单，72h 争议窗内可发起争议（非失败）",
    };
  }
  return {
    status: "done",
    receipt: { reqId, appended: true, estimated: false, disputeWindowSecs: DONE_DISPUTE_SECS },
    sseCount: 9,
    usage: { input: 1234, output: 567 },
    message: null,
  };
}

export function recordBorrowReceipt(
  state: MockBorrowSettleState,
  req: LlmBorrowRequest,
  reqId: string,
  report: LlmBorrowReport,
  tsSecs: number,
  selfPeerId: string,
): void {
  const period = new Date(tsSecs * 1000).toISOString().slice(0, 7);
  const input = report.usage?.input ?? 0;
  const output = report.usage?.output ?? 0;
  state.ledger.push({
    reqId,
    period,
    lender: req.targetPeer,
    borrower: selfPeerId,
    model: req.model,
    input,
    output,
    tokens: input + output,
    estimated: report.receipt.estimated,
    ts: tsSecs,
  });
  state.receipts.set(reqId, {
    verdict: "PASS",
    reason: "验签通过",
    reqId,
    period,
    lender: req.targetPeer,
    borrower: selfPeerId,
    model: req.model,
    input,
    output,
    estimated: report.receipt.estimated,
    ts: tsSecs,
  });
}
