import { beforeEach, describe, expect, it } from "vitest";

import type { LlmBorrowRejectionCode, LlmOfferStatus } from "./ipc-types";
import { createMockLlmShare } from "./mock-llm-share";

const SELF = "3xY9abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQ";
const LENDER = "52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd";
const REJECTION_CODES: LlmBorrowRejectionCode[] = [
  "not_allowlisted",
  "model_not_served",
  "freeze_insufficient",
  "concurrency_exceeded",
];

let nowMs = Date.UTC(2026, 8, 15, 12, 0, 0);
const { backend, controller } = createMockLlmShare({
  selfPeerId: () => SELF,
  nowMs: () => nowMs,
});

const OFFER_INPUT = {
  models: ["gpt-4o", "deepseek-v3"],
  spare: { "gpt-4o": 1500000, "deepseek-v3": 999999999 },
  periodEnds: "2026-09-30",
  maxPerReq: { "gpt-4o": 128000 },
};

const BORROW_REQ = {
  model: "gpt-4o",
  messages: [{ role: "user" as const, content: "hi" }],
  maxTokens: 1024,
  targetPeer: LENDER,
};

beforeEach(() => {
  controller.reset();
});

describe("mock-llm-share 九命令同签名往返", () => {
  it("publish/show/allow/allowList/deny/borrow/ledgerList/ledgerBalance/receiptVerify 全链路", async () => {
    const published = await backend.llmShareOfferPublish(OFFER_INPUT);
    expect(published.status).toBe("live");
    expect(published.peer).toBe(SELF);
    expect(published.file).toMatch(/offer\.json$/);

    const shown = await backend.llmShareOfferShow();
    expect(shown.models).toEqual(OFFER_INPUT.models);
    expect(shown.remainingSecs).toBe(3600);
    expect(shown.rateLimit).toEqual({ rpm: 10, concurrency: 2 });

    const allowed = await backend.llmShareAllow(LENDER, ["gpt-4o"], "首批白名单");
    expect(allowed.lastOp).toMatchObject({ op: "allow", ok: true, created: true });
    const listed = await backend.llmShareAllowList();
    expect(listed.entries).toHaveLength(1);
    expect(listed.entries[0]).toMatchObject({ peerId: LENDER, models: ["gpt-4o"] });

    const report = await backend.llmShareBorrow(BORROW_REQ);
    expect(report.status).toBe("done");
    expect(report.receipt.appended).toBe(true);
    expect(report.usage).toEqual({ input: 1234, output: 567 });

    const entries = await backend.llmShareLedgerList({ lender: LENDER });
    expect(entries).toHaveLength(1);
    expect(entries[0]).toMatchObject({ lender: LENDER, borrower: SELF, period: "2026-09" });

    const balance = await backend.llmShareLedgerBalance();
    expect(balance).toEqual([
      { lender: LENDER, period: "2026-09", netAmount: -1801, direction: "borrowed" },
    ]);

    const verified = await backend.llmShareReceiptVerify(report.receipt.reqId);
    expect(verified.verdict).toBe("PASS");

    const denied = await backend.llmShareDeny(LENDER);
    expect(denied.lastOp).toMatchObject({ op: "deny", ok: true });
    expect((await backend.llmShareAllowList()).entries).toHaveLength(0);
  });

  it("borrow 缺 targetPeer/maxTokens 缺省路径在 IPC 层显式报错", async () => {
    await expect(
      backend.llmShareBorrow({ ...BORROW_REQ, targetPeer: "" }),
    ).rejects.toThrow(/targetPeer/);
    await expect(
      backend.llmShareBorrow({ ...BORROW_REQ, maxTokens: 0 }),
    ).rejects.toThrow(/maxTokens/);
    await expect(
      backend.llmShareBorrow({ ...BORROW_REQ, targetPeer: "not-base58!" }),
    ).rejects.toThrow(/PeerId 非法/);
  });

  it("req_id 幂等：重试复用同 reqId 返回原收据且 appended=false 不双记", async () => {
    const reqId = "0198c0de-0000-7000-8000-000000000001";
    const first = await backend.llmShareBorrow({ ...BORROW_REQ, reqId });
    expect(first.receipt.appended).toBe(true);
    const retry = await backend.llmShareBorrow({ ...BORROW_REQ, reqId });
    expect(retry.receipt.appended).toBe(false);
    expect(retry.usage).toEqual(first.usage);
    expect(await backend.llmShareLedgerList()).toHaveLength(1);
  });
});

describe("mock-llm-share offer 五态与发布校验（数据面驱动）", () => {
  it("从未发布 offer show 显式报错（对齐 CLI：从未发布 → 报错非错误态缺省）", async () => {
    await expect(backend.llmShareOfferShow()).rejects.toThrow(/尚未发布/);
  });

  it.each<LlmOfferStatus>(["expired", "not_yet_valid", "peer_mismatch", "bad_signature"])(
    "控制器钉相位 %s 后 show 原样透出该 status",
    async (phase) => {
      await backend.llmShareOfferPublish(OFFER_INPUT);
      controller.setOfferPhase(phase);
      expect((await backend.llmShareOfferShow()).status).toBe(phase);
    },
  );

  it("publish 必填集校验：models 空/spare 未覆盖/引用未声明/日期非法均显式报错", async () => {
    await expect(backend.llmShareOfferPublish({ ...OFFER_INPUT, models: [] })).rejects.toThrow(/models/);
    await expect(
      backend.llmShareOfferPublish({ ...OFFER_INPUT, spare: { "gpt-4o": 100 } }),
    ).rejects.toThrow(/spare 未覆盖/);
    await expect(
      backend.llmShareOfferPublish({ ...OFFER_INPUT, spare: { "gpt-4o": 1, "deepseek-v3": 1, ghost: 5 } }),
    ).rejects.toThrow(/未声明模型/);
    await expect(
      backend.llmShareOfferPublish({ ...OFFER_INPUT, periodEnds: "2026/09/30" }),
    ).rejects.toThrow(/YYYY-MM-DD/);
  });
});

describe("mock-llm-share borrow 拒绝码四值与 stream_broken 中性语义", () => {
  it.each(REJECTION_CODES)("拒绝码 %s 原样透出不本地化改写", async (code) => {
    controller.setRejection(code);
    const report = await backend.llmShareBorrow(BORROW_REQ);
    expect(report.status).toBe("rejected");
    expect(report.code).toBe(code);
    expect(report.receipt.appended).toBe(false);
    expect(report.usage).toBeNull();
    expect(await backend.llmShareLedgerList()).toHaveLength(0);
    controller.setRejection(null);
  });

  it("stream_broken：estimated=true、72h 争议窗、正常 resolve 非命令错误", async () => {
    controller.setStreamBroken(true);
    const report = await backend.llmShareBorrow(BORROW_REQ);
    expect(report.status).toBe("stream_broken");
    expect(report.receipt.estimated).toBe(true);
    expect(report.receipt.disputeWindowSecs).toBe(72 * 3600);
    expect(report.receipt.appended).toBe(true);
    const entries = await backend.llmShareLedgerList();
    expect(entries[0]?.estimated).toBe(true);
    // §16.2.6：sse 原文不透传，形状上仅存在计数。
    expect(Object.keys(report)).not.toContain("sseText");
  });

  it("receipt verify 公钥不绑定给 FAIL 判定；未知 req_id 显式报错", async () => {
    const { receipt } = await backend.llmShareBorrow(BORROW_REQ);
    const fail = await backend.llmShareReceiptVerify(receipt.reqId, "wrongpubkey");
    expect(fail.verdict).toBe("FAIL");
    expect(fail.reason).toMatch(/不绑定/);
    await expect(backend.llmShareReceiptVerify("no-such-req")).rejects.toThrow(/req_id 未知/);
  });
});

describe("mock-llm-share allowlist 默认拒绝语义", () => {
  it("deny 不存在条目 = ok:false 显式报错非错误态", async () => {
    const view = await backend.llmShareDeny(LENDER);
    expect(view.lastOp?.ok).toBe(false);
    expect(view.lastOp?.message).toMatch(/默认拒绝/);
  });

  it("allow upsert：重复 allow 刷新 grantedAt 且 created=false；缺 models = 不限模型", async () => {
    await backend.llmShareAllow(LENDER, ["gpt-4o"]);
    const firstGrantedAt = (await backend.llmShareAllowList()).entries[0]?.grantedAt;
    nowMs += 5000;
    const again = await backend.llmShareAllow(LENDER);
    expect(again.lastOp?.created).toBe(false);
    const entry = (await backend.llmShareAllowList()).entries[0];
    expect(entry?.models).toBeNull();
    expect(entry?.grantedAt).not.toBe(firstGrantedAt);
  });
});
