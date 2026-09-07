import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import "@/i18n";
import i18n from "@/i18n";
import { formatDateTime } from "@/lib/format";
import { shortPeerId } from "@/lib/peer-name";
import { useChatStore } from "@/stores/chat-store";

import { makeLlmShareMockPair, MOCK_LOCAL_PEER } from "./mock-backend";
import { LedgerPanel } from "./ledger-panel";
import type { LlmLedgerEntry } from "./types";

const t = i18n.t.bind(i18n);
const PEER = "52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd";
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

function seedTwoPeriods(mock: ReturnType<typeof makeLlmShareMockPair>["mock"]) {
  mock.seedLedgerEntry(entry({ reqId: "own", tokens: 100 }));
  mock.seedLedgerEntry(
    entry({ reqId: "in", lender: PEER, borrower: MOCK_LOCAL_PEER, tokens: 300 }),
  );
  mock.seedLedgerEntry(
    entry({ reqId: "old", lender: PEER, borrower: MOCK_LOCAL_PEER, tokens: 40, period: "2026-08" }),
  );
}

afterEach(() => cleanup());

describe("LLM3 双边账本面板（§16.1 balance 分组方向徽标 / list 三过滤 / receipt verify）", () => {
  it("净差按 lender+period 分组，方向徽标与正负号对齐", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    seedTwoPeriods(mock);
    render(<LedgerPanel backend={backend} />);
    const rows = await screen.findAllByTestId("balance-row");
    expect(rows).toHaveLength(3);
    expect(rows[0].getAttribute("data-direction")).toBe("lent");
    expect(rows[0].textContent).toContain("+100");
    expect(rows[1].getAttribute("data-direction")).toBe("borrowed");
    expect(rows[1].textContent).toContain("-300");
    expect(rows[2].textContent).toContain("-40");
    expect(rows[0].textContent).toContain(t("llmShare.ledger.directionLent"));
    expect(rows[1].textContent).toContain(t("llmShare.ledger.directionBorrowed"));
  });

  it("流水三过滤器：period 过滤后仅剩对应账期条目", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    seedTwoPeriods(mock);
    render(<LedgerPanel backend={backend} />);
    expect(await screen.findAllByTestId("ledger-row")).toHaveLength(3);
    fireEvent.change(screen.getByLabelText(t("llmShare.ledger.filterPeriod")), {
      target: { value: "2026-08" },
    });
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.ledger.applyFilter") }));
    await waitFor(() =>
      expect(screen.getAllByTestId("ledger-row")).toHaveLength(1),
    );
    expect(screen.getAllByTestId("ledger-row")[0].textContent).toContain("old");
  });

  // R2-10 回归：流水表补时间列与「共 N 条 · 合计 X tokens」统计行
  it("流水表渲染本地化时间列与检索规模统计行", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    seedTwoPeriods(mock);
    render(<LedgerPanel backend={backend} />);
    expect(await screen.findByRole("columnheader", { name: t("llmShare.ledger.columnTs") })).toBeTruthy();
    const firstTs = screen.getAllByTestId("ledger-row-ts")[0];
    expect(firstTs.textContent).toBe(formatDateTime(T0 * 1000, i18n.language as "zh-CN"));
    expect(firstTs.textContent).not.toMatch(/T0|Z/u);
    const stats = await screen.findByTestId("ledger-stats");
    expect(stats.textContent).toBe(t("llmShare.ledger.statsLine", { count: 3, tokens: 440 }));
  });

  // R2-11 回归：净差行组合值拆标签（出借/借入/笔数），lender 走昵称映射
  it("净差行组合值拆带标签明细，lender 显示昵称映射", async () => {
    useChatStore.setState({
      friends: [{ peerId: PEER, nickname: "小借", addrs: [] }],
      friendsLoaded: true,
    });
    const { backend, mock } = makeLlmShareMockPair();
    seedTwoPeriods(mock);
    render(<LedgerPanel backend={backend} />);
    const rows = await screen.findAllByTestId("balance-row");
    const own = rows.find((row) => row.getAttribute("data-direction") === "lent");
    expect(own?.querySelector('[data-testid="balance-detail"]')?.textContent).toBe(
      t("llmShare.ledger.balanceDetail", { lentOut: 100, borrowed: 0, entries: 1 }),
    );
    const borrowed = rows.find((row) => row.getAttribute("data-direction") === "borrowed");
    expect(borrowed?.textContent).toContain(`小借 (${shortPeerId(PEER)})`);
    useChatStore.setState({ friends: [], friendsLoaded: true });
  });

  it("收据验核：缺省本机身份 PASS；高级字段 lenderPubkey 错值 FAIL", async () => {
    const { backend, mock } = makeLlmShareMockPair({ now: () => T0 });
    mock.allow({ peerId: PEER });
    mock.borrow({ targetPeer: PEER, model: "gpt-4o", messages: "hi", maxTokens: 8, reqId: "R1" });
    render(<LedgerPanel backend={backend} />);
    fireEvent.change(screen.getByLabelText(t("llmShare.ledger.verifyReqId")), {
      target: { value: "R1" },
    });
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.ledger.verify") }));
    let result = await screen.findByTestId("verify-result");
    expect(result.getAttribute("data-verdict")).toBe("PASS");
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.ledger.advancedToggle") }));
    fireEvent.change(screen.getByLabelText(t("llmShare.ledger.advancedLenderPubkey")), {
      target: { value: "TkQj3nGvArK5vjNKPz7eSC9y3csxncb8o29KCTaxvZkWRONG" },
    });
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.ledger.verify") }));
    result = await screen.findByTestId("verify-result");
    expect(result.getAttribute("data-verdict")).toBe("FAIL");
    expect(result.textContent).toContain(t("llmShare.ledger.verdictFail"));
  });
});
