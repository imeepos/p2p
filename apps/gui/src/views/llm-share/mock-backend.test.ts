import { describe, expect, it } from "vitest";

import {
  LlmShareMock,
  LlmShareMockError,
  MOCK_LENDER_PUBKEY,
  MOCK_LOCAL_PEER,
} from "./mock-backend";
import type { LlmLedgerEntry } from "./types";

// mock 语义基线：逐条目映射 ai-guide llm-share 九条目 + 契约 §16.2 约束，
// 供 PR 轨跨轨会签逐字核对（测试名即映射点）。
const PEER = "52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd";
const OTHER = "3wqibX4fA7GvGC1gpGZnEfBEYZPcx4DFnGEBi3yYb2Jv";
const T0 = 1788549300;

function entry(overrides: Partial<LlmLedgerEntry>): LlmLedgerEntry {
  return {
    reqId: "req-x",
    period: "2026-09",
    lender: MOCK_LOCAL_PEER,
    borrower: PEER,
    model: "gpt-4o",
    input: 10,
    output: 20,
    tokens: 30,
    estimated: false,
    ts: T0,
    ...overrides,
  };
}

describe("llm-share allow/allowlist/deny（ai-guide 条目 1-3）", () => {
  it("allow upsert 缺省不限模型；重复 allow 更新并刷新 grantedAt；deny 缺条目显式报错", () => {
    const mock = new LlmShareMock({ now: () => T0 });
    mock.allow({ peerId: PEER });
    expect(mock.allowList().entries[0].models).toEqual([]);
    mock.allow({ peerId: PEER, models: ["gpt-4o"], note: "白名单" });
    const [row] = mock.allowList().entries;
    expect(row.models).toEqual(["gpt-4o"]);
    expect(row.note).toBe("白名单");
    expect(() => mock.deny(OTHER)).toThrow(LlmShareMockError);
  });

  it("allowlist 空=默认拒绝心智（§16.2-7）：无条目借方一律不可用", () => {
    const mock = new LlmShareMock();
    expect(mock.allowList().entries).toEqual([]);
    const report = mock.borrow({
      targetPeer: PEER,
      model: "gpt-4o",
      messages: "hi",
      maxTokens: 128,
    });
    expect(report.status).toBe("rejected");
    expect(report.code).toBe("not_allowlisted");
    expect(report.receipt.appended).toBe(false);
    expect(mock.ledgerList()).toHaveLength(0);
  });
});

describe("llm-share offer publish/show（ai-guide 条目 4-5）", () => {
  it("publish 必填集校验：models≥1 / spare 覆盖且 N>0 / 不引用未声明模型 / 日期格式", () => {
    const mock = new LlmShareMock({ now: () => T0 });
    const base = { spare: { "gpt-4o": 10 }, periodEnds: "2026-09-30" };
    expect(() => mock.offerPublish({ models: [], ...base })).toThrow(/model/);
    expect(() => mock.offerPublish({ models: ["gpt-4o"], spare: {}, periodEnds: "2026-09-30" })).toThrow(/spare/);
    expect(() => mock.offerPublish({ models: ["gpt-4o"], spare: { "gpt-4o": 0 }, periodEnds: "2026-09-30" })).toThrow(/spare/);
    expect(() => mock.offerPublish({ models: ["gpt-4o"], spare: { "gpt-4o": 5, m2: 5 }, periodEnds: "2026-09-30" })).toThrow(/undeclared/);
    expect(() => mock.offerPublish({ models: ["gpt-4o"], ...base, periodEnds: "0930" })).toThrow(/invalid date/);
  });

  it("show 状态机：live → expired / 时钟回拨 not_yet_valid / force 警示态", () => {
    let now = T0;
    const mock = new LlmShareMock({ now: () => now });
    mock.offerPublish({ models: ["gpt-4o"], spare: { "gpt-4o": 5 }, periodEnds: "2026-09-30" });
    expect(mock.offerShow().status).toBe("live");
    now = T0 + 3601;
    expect(mock.offerShow().status).toBe("expired");
    now = T0 - 10;
    expect(mock.offerShow().status).toBe("not_yet_valid");
    const forced = new LlmShareMock({ forceOfferStatus: "bad_signature", now: () => T0 });
    forced.offerPublish({ models: ["gpt-4o"], spare: { "gpt-4o": 5 }, periodEnds: "2026-09-30" });
    expect(forced.offerShow().status).toBe("bad_signature");
  });
});

describe("llm-share borrow（ai-guide 条目 9，§16.2-1/2/3/6）", () => {
  it("模型未开放 → rejected model_not_served；四值拒绝码原样透出不改写", () => {
    const mock = new LlmShareMock();
    mock.allow({ peerId: PEER, models: ["gpt-4o"] });
    const report = mock.borrow({ targetPeer: PEER, model: "m2", messages: "hi", maxTokens: 16 });
    expect(report.status).toBe("rejected");
    expect(report.code).toBe("model_not_served");
    expect(mock.ledgerList()).toHaveLength(0);
  });

  it("done：收据入账 + usage + sse 正文截断预览（只计数不透传原文）", () => {
    const mock = new LlmShareMock({ now: () => T0, previewChars: 20 });
    mock.allow({ peerId: PEER });
    const report = mock.borrow({ targetPeer: PEER, model: "gpt-4o", messages: "hi", maxTokens: 128 });
    expect(report.status).toBe("done");
    expect(report.receipt.estimated).toBe(false);
    expect(report.receipt.disputeWindowSecs).toBe(24 * 3600);
    expect(report.usage).toEqual({ input: 1234, output: 567 });
    expect(report.sseCount).toBeGreaterThan(0);
    expect(report.message?.endsWith("... (truncated)")).toBe(true);
    expect(mock.ledgerList()).toHaveLength(1);
  });

  it("§16.2-3 reqId 幂等：重放 appended=false 不双记账，调用序可断言", () => {
    const mock = new LlmShareMock({ now: () => T0 });
    mock.allow({ peerId: PEER });
    const first = mock.borrow({ targetPeer: PEER, model: "gpt-4o", messages: "hi", maxTokens: 8, reqId: "R1" });
    const replay = mock.borrow({ targetPeer: PEER, model: "gpt-4o", messages: "hi", maxTokens: 8, reqId: "R1" });
    expect(first.receipt.appended).toBe(true);
    expect(replay.receipt.appended).toBe(false);
    expect(mock.ledgerList()).toHaveLength(1);
    expect(mock.borrowReqIds).toEqual(["R1", "R1"]);
  });

  it("§16.2-2 stream_broken：estimated=true + 72h 争议窗，非错误照常结算入账", () => {
    const mock = new LlmShareMock({ now: () => T0, breakStreamEveryBorrow: true });
    mock.allow({ peerId: PEER });
    const report = mock.borrow({ targetPeer: PEER, model: "gpt-4o", messages: "hi", maxTokens: 8 });
    expect(report.status).toBe("stream_broken");
    expect(report.receipt.estimated).toBe(true);
    expect(report.receipt.disputeWindowSecs).toBe(72 * 3600);
    expect(report.receipt.appended).toBe(true);
    expect(mock.ledgerList()[0].estimated).toBe(true);
  });

  it("参数校验：必填缺失 / maxTokens 非正整数显式抛错不静默", () => {
    const mock = new LlmShareMock();
    mock.allow({ peerId: PEER });
    const valid = { targetPeer: PEER, model: "gpt-4o", messages: "hi", maxTokens: 8 };
    expect(() => mock.borrow({ ...valid, targetPeer: " " })).toThrow(/targetPeer/);
    expect(() => mock.borrow({ ...valid, maxTokens: 0 })).toThrow(/maxTokens/);
    expect(() => mock.borrow({ ...valid, maxTokens: 1.5 })).toThrow(/maxTokens/);
  });
});

describe("llm-share ledger list/balance（ai-guide 条目 6-7）", () => {
  it("list：lender/borrower/period 三参数过滤", () => {
    const mock = new LlmShareMock();
    mock.seedLedgerEntry(entry({ reqId: "a" }));
    mock.seedLedgerEntry(entry({ reqId: "b", period: "2026-10" }));
    mock.seedLedgerEntry(entry({ reqId: "c", lender: PEER, borrower: OTHER }));
    expect(mock.ledgerList({ lender: MOCK_LOCAL_PEER }).map((e) => e.reqId)).toEqual(["a", "b"]);
    expect(mock.ledgerList({ period: "2026-10" }).map((e) => e.reqId)).toEqual(["b"]);
    expect(mock.ledgerList({ borrower: OTHER }).map((e) => e.reqId)).toEqual(["c"]);
  });

  it("balance：lender+period 分组，本机 lender 正 / borrower 负，他机条目不计", () => {
    const mock = new LlmShareMock();
    mock.seedLedgerEntry(entry({ reqId: "a", tokens: 100 }));
    mock.seedLedgerEntry(entry({ reqId: "b", lender: PEER, borrower: MOCK_LOCAL_PEER, tokens: 300 }));
    mock.seedLedgerEntry(entry({ reqId: "c", lender: PEER, borrower: MOCK_LOCAL_PEER, tokens: 40, period: "2026-10" }));
    mock.seedLedgerEntry(entry({ reqId: "d", lender: OTHER, borrower: PEER, tokens: 999 }));
    const rows = mock.ledgerBalance();
    expect(rows).toHaveLength(3);
    const own = rows.find((r) => r.lender === MOCK_LOCAL_PEER);
    expect(own).toMatchObject({ netAmount: 100, direction: "lent", entries: 1 });
    const borrowed = rows.find((r) => r.lender === PEER && r.period === "2026-09");
    expect(borrowed).toMatchObject({ netAmount: -300, direction: "borrowed", lentOut: 0, borrowed: 300 });
    const otherPeriod = rows.find((r) => r.period === "2026-10");
    expect(otherPeriod).toMatchObject({ netAmount: -40, direction: "borrowed" });
  });
});

describe("llm-share receipt verify（ai-guide 条目 8）", () => {
  it("PASS / FAIL / 公钥不绑定 / 未知收据 / 篡改名单", () => {
    const mock = new LlmShareMock({
      now: () => T0,
      failVerifyReqIds: ["R-tamper"],
    });
    mock.allow({ peerId: PEER });
    mock.borrow({ targetPeer: PEER, model: "gpt-4o", messages: "hi", maxTokens: 8, reqId: "R1" });
    mock.borrow({ targetPeer: PEER, model: "gpt-4o", messages: "hi", maxTokens: 8, reqId: "R-tamper" });
    expect(mock.receiptVerify({ reqId: "R1" }).verdict).toBe("PASS");
    expect(mock.receiptVerify({ reqId: "R1" }).reason).toBe("signature verified");
    expect(mock.receiptVerify({ reqId: "R1", lenderPubkey: "bad" }).verdict).toBe("FAIL");
    expect(mock.receiptVerify({ reqId: "R1", lenderPubkey: MOCK_LENDER_PUBKEY }).verdict).toBe("PASS");
    expect(mock.receiptVerify({ reqId: "nope" }).verdict).toBe("FAIL");
    const tampered = mock.receiptVerify({ reqId: "R-tamper" });
    expect(tampered.verdict).toBe("FAIL");
    expect(tampered.reason).toContain("signature invalid");
  });
});
