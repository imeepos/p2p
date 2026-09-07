import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import "@/i18n";
import i18n from "@/i18n";

import { BorrowReportCard } from "./borrow-report";
import type { LlmBorrowReport } from "./types";

const t = i18n.t.bind(i18n);

function report(overrides: Partial<LlmBorrowReport>): LlmBorrowReport {
  return {
    status: "done",
    receipt: { reqId: "req-1", appended: true, estimated: false, disputeWindowSecs: 86400 },
    sseCount: 9,
    usage: { input: 10, output: 20 },
    message: '{"choices":[{"delta":{"content":"hello"}}]}',
    ...overrides,
  };
}

afterEach(() => cleanup());

// R2-07 回归：九项内部术语泄漏逐条改用户语言；拒绝码保留并入可复制详情
describe("R2-07 借用报告卡用户语言（术语不泄漏）", () => {
  it("字段名与取值用用户语言：请求编号/入账状态/争议窗口/SSE 事件数", () => {
    render(<BorrowReportCard report={report({})} />);
    const text = screen.getByTestId("borrow-report").textContent ?? "";
    expect(text).toContain(t("llmShare.borrow.reqIdLabel"));
    expect(text).toContain(t("llmShare.borrow.appendedYes"));
    expect(text).toContain(t("llmShare.borrow.disputeWindowLabel"));
    expect(text).toContain(t("llmShare.borrow.disputeWindowValue", { hours: 24 }));
    expect(text).toContain(t("llmShare.borrow.sseCountLabel"));
    for (const leak of ["disputeWindowSecs", "appended", "reqId", "SSE 帧数"]) {
      expect(text).not.toContain(leak);
    }
  });

  it("拒绝态：人话原因 + 拒绝码保留 + 复制详情按钮，内部英文句不露", () => {
    render(
      <BorrowReportCard
        report={report({
          status: "rejected",
          code: "not_allowlisted",
          message: "upstream untouched: borrow rejected structurally (default-deny or model not served)",
        })}
      />,
    );
    const text = screen.getByTestId("borrow-report").textContent ?? "";
    expect(text).toContain(t("llmShare.borrow.rejectNotAllowlisted"));
    expect(screen.getByTestId("reject-code").textContent).toBe("not_allowlisted");
    expect(text).not.toContain("upstream untouched");
    expect(screen.getByRole("button", { name: t("common.feedback.copyDetail") })).toBeTruthy();
  });

  it("未入账（重放 appended=false）显示未入账而非裸布尔", () => {
    render(
      <BorrowReportCard
        report={report({
          receipt: { reqId: "req-1", appended: false, estimated: false, disputeWindowSecs: 0 },
        })}
      />,
    );
    const text = screen.getByTestId("borrow-report").textContent ?? "";
    expect(text).toContain(t("llmShare.borrow.appendedNo"));
    expect(text).not.toContain("false");
    expect(text).not.toContain(t("llmShare.borrow.disputeWindowLabel"));
  });

  it("拒绝码到人话原因映射四码齐备", () => {
    for (const code of [
      "not_allowlisted",
      "model_not_served",
      "freeze_insufficient",
      "concurrency_exceeded",
    ] as const) {
      render(<BorrowReportCard report={report({ status: "rejected", code })} />);
      expect(
        screen.getByTestId("reject-reason").textContent,
      ).not.toContain("undefined");
      cleanup();
    }
  });
});
