import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import "@/i18n";
import i18n from "@/i18n";

import { makeLlmShareMockPair } from "./mock-backend";
import { BorrowPanel } from "./borrow-panel";

const t = i18n.t.bind(i18n);
const PEER = "52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd";

function renderPanel(backend: Parameters<typeof BorrowPanel>[0]["backend"]) {
  return render(
    <ConfirmProvider>
      <BorrowPanel backend={backend} />
    </ConfirmProvider>,
  );
}

function formOf(button: HTMLElement): HTMLFormElement {
  const form = button.closest("form");
  if (!form) throw new Error("test precondition broken: borrow form missing");
  return form;
}

function fillValidForm() {
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
  fireEvent.submit(formOf(screen.getByRole("button", { name: t("llmShare.borrow.submit") })));
  await screen.findByRole("alertdialog");
  fireEvent.click(screen.getByRole("button", { name: t("common.actions.confirm") }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
}

function confirmDescription(): string {
  return `${t("llmShare.borrow.confirmBody")} ${t("llmShare.borrow.costLine", {
    model: "gpt-4o",
    maxTokens: 128,
    peer: PEER,
  })}`;
}

afterEach(() => cleanup());

describe("LLM3 borrow 面板（§16.2-3/6 二次确认 + reqId 幂等复用）", () => {
  it("二次确认拦截：取消不触达后端；确认描述明示真实成本后才调用", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    mock.allow({ peerId: PEER });
    const spy = vi.spyOn(backend, "borrow");
    renderPanel(backend);
    fillValidForm();
    fireEvent.submit(formOf(screen.getByRole("button", { name: t("llmShare.borrow.submit") })));
    expect(await screen.findByRole("alertdialog")).toBeTruthy();
    expect(screen.getByText(confirmDescription())).toBeTruthy();
    expect(spy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: t("common.actions.cancel") }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(spy).not.toHaveBeenCalled();
    await submitThroughConfirm();
    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy.mock.calls[0][0].maxTokens).toBe(128);
    expect(await screen.findByTestId("borrow-report")).toBeTruthy();
  });

  it("必填校验拦截：错误内联展示，后端零调用（maxTokens 有厂值缺省不再必填）", async () => {
    const { backend } = makeLlmShareMockPair();
    const spy = vi.spyOn(backend, "borrow");
    renderPanel(backend);
    fireEvent.submit(formOf(screen.getByRole("button", { name: t("llmShare.borrow.submit") })));
    const alerts = await screen.findAllByRole("alert");
    expect(alerts).toHaveLength(3);
    expect(spy).not.toHaveBeenCalled();
  });

  it("§16.2-3 重试复用同 reqId（rejected 后重试同一请求意图）", async () => {
    const { backend, mock } = makeLlmShareMockPair({ rejectCode: "not_allowlisted" });
    const spy = vi.spyOn(backend, "borrow");
    renderPanel(backend);
    fillValidForm();
    await submitThroughConfirm();
    expect((await screen.findByTestId("reject-code")).textContent).toBe("not_allowlisted");
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.borrow.retry") }));
    await screen.findByRole("alertdialog");
    fireEvent.click(screen.getByRole("button", { name: t("common.actions.confirm") }));
    await waitFor(() => expect(spy).toHaveBeenCalledTimes(2));
    expect(mock.borrowReqIds[0]).toBe(mock.borrowReqIds[1]);
    expect(mock.borrowReqIds[0]).toMatch(/^[0-9a-f-]{36}$/);
  });

  it("§16.2-2 stream_broken 渲染「估算账单·72h 争议窗」中性态，非失败非 alert", async () => {
    const { backend, mock } = makeLlmShareMockPair({ breakStreamEveryBorrow: true });
    mock.allow({ peerId: PEER });
    renderPanel(backend);
    fillValidForm();
    await submitThroughConfirm();
    const report = await screen.findByTestId("borrow-report");
    expect(report.getAttribute("data-tone")).toBe("neutral");
    expect(report.textContent).toContain(t("llmShare.borrow.streamBrokenBadge"));
    expect(report.textContent).toContain(t("llmShare.borrow.disputeWindowValue", { hours: 72 }));
    expect(report.textContent).not.toContain("259200");
    expect(report.querySelector('[role="alert"]')).toBeNull();
  });

  it("rejected 是业务结果非错误态：拒绝码原样透出不本地化", async () => {
    const { backend } = makeLlmShareMockPair({ rejectCode: "freeze_insufficient" });
    renderPanel(backend);
    fillValidForm();
    await submitThroughConfirm();
    expect((await screen.findByTestId("reject-code")).textContent).toBe("freeze_insufficient");
    const report = await screen.findByTestId("borrow-report");
    expect(report.getAttribute("data-tone")).toBe("warning");
    expect(report.getAttribute("data-tone")).not.toBe("danger");
  });

  it("后端异常路径显式露出（role=alert），不渲染成结果卡片", async () => {
    const { backend } = makeLlmShareMockPair();
    const spy = {
      ...backend,
      borrow: () => Promise.reject(new Error("connection refused")),
    };
    renderPanel(spy);
    fillValidForm();
    await submitThroughConfirm();
    expect(await screen.findByRole("alert")).toHaveTextContent("connection refused");
  });

  // R2-06 回归：model 选择器数据源 = 白名单已放行模型集，选中回填输入框
  it("model 选择器候选来自白名单模型集，选中即回填", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    mock.allow({ peerId: PEER, models: ["gpt-4o", "deepseek-v3"] });
    renderPanel(backend);
    fireEvent.click(await screen.findByTestId("llm-borrow-model-pick"));
    fireEvent.click(await screen.findByTestId("llm-borrow-model-pick-panel"));
    fireEvent.click(screen.getByRole("option", { name: "deepseek-v3" }));
    const input = screen.getByLabelText(t("llmShare.borrow.formModel")) as HTMLInputElement;
    expect(input.value).toBe("deepseek-v3");
  });

  it("maxTokens/messages 补占位与辅助说明", () => {
    const { backend } = makeLlmShareMockPair();
    renderPanel(backend);
    const maxTokens = screen.getByLabelText(t("llmShare.borrow.formMaxTokens"));
    expect(maxTokens.getAttribute("placeholder")).toBe(
      t("llmShare.borrow.formMaxTokensPlaceholder"),
    );
    expect(maxTokens.getAttribute("aria-describedby")).toBe("llm-borrow-maxtokens-hint");
    expect(
      document.getElementById("llm-borrow-maxtokens-hint")?.textContent,
    ).toContain(t("llmShare.borrow.formMaxTokensHint"));
    const messages = screen.getByLabelText(t("llmShare.borrow.formMessages"));
    expect(messages.getAttribute("placeholder")).toBe(
      t("llmShare.borrow.formMessagesPlaceholder"),
    );
    expect(
      document.getElementById("llm-borrow-messages-hint")?.textContent,
    ).toContain(t("llmShare.borrow.formMessagesHint"));
  });
});
