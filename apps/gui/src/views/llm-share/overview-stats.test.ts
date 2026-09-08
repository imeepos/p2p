import { describe, expect, it } from "vitest";

import { recentEntries, summarizeLedger } from "./overview-stats";
import type { LlmBalanceGroup, LlmLedgerEntry } from "./types";

function entry(overrides: Partial<LlmLedgerEntry>): LlmLedgerEntry {
  return {
    reqId: "r",
    period: "2026-09",
    lender: "l",
    borrower: "b",
    model: "gpt-4o",
    input: 1,
    output: 2,
    tokens: 3,
    estimated: false,
    ts: 1,
    ...overrides,
  };
}

describe("summarizeLedger（概览统计汇总）", () => {
  it("空数据归零", () => {
    expect(summarizeLedger([], [])).toEqual({
      entriesCount: 0,
      totalTokens: 0,
      lentOut: 0,
      borrowed: 0,
    });
  });

  it("流水笔数与 tokens 合计、净差双向求和", () => {
    const entries = [
      entry({ reqId: "r1", tokens: 10 }),
      entry({ reqId: "r2", tokens: 15 }),
    ];
    const groups: LlmBalanceGroup[] = [
      { lender: "l", period: "2026-09", lentOut: 100, borrowed: 0, netAmount: 100, entries: 2, direction: "lent" },
      { lender: "l", period: "2026-08", lentOut: 0, borrowed: 40, netAmount: -40, entries: 1, direction: "borrowed" },
    ];
    expect(summarizeLedger(entries, groups)).toEqual({
      entriesCount: 2,
      totalTokens: 25,
      lentOut: 100,
      borrowed: 40,
    });
  });
});

describe("recentEntries（最近流水截取）", () => {
  it("按时间倒序取前 limit 条", () => {
    const rows = [
      entry({ reqId: "old", ts: 1 }),
      entry({ reqId: "new", ts: 9 }),
      entry({ reqId: "mid", ts: 5 }),
    ];
    expect(recentEntries(rows, 2).map((r) => r.reqId)).toEqual(["new", "mid"]);
  });

  it("limit 超过总数时全量返回", () => {
    const rows = [entry({ reqId: "only", ts: 1 })];
    expect(recentEntries(rows, 5)).toHaveLength(1);
  });
});
