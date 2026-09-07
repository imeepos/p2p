import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import "@/i18n";
import i18n from "@/i18n";

import { makeLlmShareMockPair } from "./mock-backend";
import { BorrowPanel } from "./borrow-panel";
import { LedgerPanel } from "./ledger-panel";

const t = i18n.t.bind(i18n);
const PEER = "52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd";

// R2-01 回归：借用入账后账本卡联动刷新（借用成功事件驱动 + 「查询」联动净差
// + 净差卡显式刷新入口），不允许再出现「流水已出、净差卡停留旧空态」的断链。
function fillBorrowForm() {
  fireEvent.change(screen.getByLabelText(t("llmShare.borrow.formTargetPeer")), {
    target: { value: PEER },
  });
  fireEvent.change(screen.getByLabelText(t("llmShare.borrow.formModel")), {
    target: { value: "gpt-4o" },
  });
  fireEvent.change(screen.getByLabelText(t("llmShare.borrow.formMaxTokens")), {
    target: { value: "128" },
  });
  fireEvent.change(screen.getByLabelText(t("llmShare.borrow.formMessages")), {
    target: { value: "你好" },
  });
}

async function submitThroughConfirm() {
  const form = screen
    .getByRole("button", { name: t("llmShare.borrow.submit") })
    .closest("form");
  if (!form) throw new Error("test precondition broken: borrow form missing");
  fireEvent.submit(form);
  await screen.findByRole("alertdialog");
  fireEvent.click(screen.getByRole("button", { name: t("common.actions.confirm") }));
  await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
}

afterEach(() => cleanup());

describe("R2-01 借用入账 → 账本三卡联动", () => {
  it("借用完成后净差与流水自动出现，无需任何手动刷新", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    mock.allow({ peerId: PEER });
    render(
      <ConfirmProvider>
        <BorrowPanel backend={backend} />
        <LedgerPanel backend={backend} />
      </ConfirmProvider>,
    );
    expect(await screen.findByText(t("llmShare.ledger.emptyBalance"))).toBeTruthy();
    fillBorrowForm();
    await submitThroughConfirm();
    expect(await screen.findByTestId("borrow-report")).toBeTruthy();
    const balanceRows = await screen.findAllByTestId("balance-row");
    expect(balanceRows).toHaveLength(1);
    expect(await screen.findByTestId("ledger-row")).toBeTruthy();
  });

  it("「查询」联动净差重拉，净差卡显式刷新按钮同样触发重拉", async () => {
    const { backend } = makeLlmShareMockPair();
    const balanceSpy = vi.spyOn(backend, "ledgerBalance");
    render(
      <ConfirmProvider>
        <BorrowPanel backend={backend} />
        <LedgerPanel backend={backend} />
      </ConfirmProvider>,
    );
    await screen.findByText(t("llmShare.ledger.emptyBalance"));
    const afterMount = balanceSpy.mock.calls.length;
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.ledger.applyFilter") }));
    await waitFor(() => expect(balanceSpy.mock.calls.length).toBe(afterMount + 1));
    fireEvent.click(screen.getByTestId("balance-refresh"));
    await waitFor(() => expect(balanceSpy.mock.calls.length).toBe(afterMount + 2));
  });

  it("拒绝路径不入账，净差卡保持空态（不误刷新成有数据）", async () => {
    const { backend } = makeLlmShareMockPair({ rejectCode: "not_allowlisted" });
    render(
      <ConfirmProvider>
        <BorrowPanel backend={backend} />
        <LedgerPanel backend={backend} />
      </ConfirmProvider>,
    );
    fillBorrowForm();
    await submitThroughConfirm();
    expect(await screen.findByTestId("reject-code")).toBeTruthy();
    await waitFor(() =>
      expect(screen.getByText(t("llmShare.ledger.emptyBalance"))).toBeTruthy(),
    );
    expect(screen.queryByTestId("balance-row")).toBeNull();
  });
});
