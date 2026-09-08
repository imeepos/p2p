import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import "@/i18n";
import i18n from "@/i18n";

import { makeLlmShareMockPair } from "./mock-backend";
import { resetOfferLoadWarnForTest } from "./offer-errors";
import { OfferPanel } from "./offer-panel";
import { BorrowPanel } from "./borrow-panel";
import { AllowlistPanel } from "./allowlist-panel";

const t = i18n.t.bind(i18n);
const PEER = "52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd";

function submitOf(buttonName: string): HTMLFormElement {
  const form = screen.getByRole("button", { name: buttonName }).closest("form");
  if (!form) throw new Error("test precondition broken: form missing");
  return form;
}

afterEach(() => {
  cleanup();
  resetOfferLoadWarnForTest();
});

// jsdom 未实现 scrollIntoView（settings-focus-error 同款 stub）
beforeEach(() => {
  HTMLElement.prototype.scrollIntoView = vi.fn();
});

describe("R2-12 校验错误 aria 关联与聚焦第一个错误字段", () => {
  it("offer 空提交：字段 aria-invalid/describedby 关联且聚焦第一个错误", async () => {
    const { backend } = makeLlmShareMockPair();
    render(<OfferPanel backend={backend} />);
    // UX：表单默认收起，先点 CTA 展开
    await screen.findByTestId("offer-publish-cta");
    fireEvent.click(screen.getByTestId("offer-publish-cta"));
    fireEvent.submit(submitOf(t("llmShare.offer.publish")));
    const models = screen.getByLabelText(t("llmShare.offer.formModels"));
    expect(models.getAttribute("aria-invalid")).toBe("true");
    expect(models.getAttribute("aria-describedby")).toBe("llm-offer-models-error");
    expect(document.getElementById("llm-offer-models-error")?.getAttribute("role")).toBe("alert");
    expect(document.activeElement?.id).toBe("llm-offer-models");
  });

  it("borrow 空提交：聚焦出借方 PeerId，必填字段均带 aria 关联（maxTokens 有厂值缺省）", async () => {
    const { backend } = makeLlmShareMockPair();
    render(
      <MemoryRouter>
        <ConfirmProvider>
          <BorrowPanel backend={backend} />
        </ConfirmProvider>
      </MemoryRouter>,
    );
    fireEvent.submit(submitOf(t("llmShare.borrow.submit")));
    expect(document.activeElement?.id).toBe("llm-borrow-peer");
    const peer = screen.getByLabelText(t("llmShare.borrow.formTargetPeer"));
    expect(peer.getAttribute("aria-invalid")).toBe("true");
    for (const id of [
      "llm-borrow-peer-error",
      "llm-borrow-model-error",
      "llm-borrow-messages-error",
    ]) {
      expect(document.getElementById(id)).toBeTruthy();
    }
  });

  it("allowlist 空提交：PeerId 即时报必填并聚焦", async () => {
    const { backend } = makeLlmShareMockPair();
    const allowSpy = vi.spyOn(backend, "allow");
    render(
      <ConfirmProvider>
        <AllowlistPanel backend={backend} />
      </ConfirmProvider>,
    );
    // UX：表单默认收起，先点「添加放行」展开
    fireEvent.click(await screen.findByTestId("allow-add-toggle"));
    fireEvent.submit(submitOf(t("llmShare.allowlist.allow")));
    const peer = screen.getByLabelText(t("llmShare.allowlist.formPeerId"));
    expect(peer.getAttribute("aria-invalid")).toBe("true");
    expect(peer.getAttribute("aria-describedby")).toBe("llm-allow-peer-error");
    expect(document.activeElement?.id).toBe("llm-allow-peer");
    expect(allowSpy).not.toHaveBeenCalled();
  });
});

describe("R2-14 借用处理中状态反馈", () => {
  it("确认后到报告卡出现之间渲染「正在调用」状态行", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    mock.allow({ peerId: PEER });
    let resolveBorrow: (value: Parameters<typeof backend.borrow>[0] extends never ? never : Awaited<ReturnType<typeof backend.borrow>>) => void = () => {};
    const deferred = new Promise<Awaited<ReturnType<typeof backend.borrow>>>((resolve) => {
      resolveBorrow = resolve as typeof resolveBorrow;
    });
    const slowBackend = { ...backend, borrow: () => deferred };
    render(
      <MemoryRouter>
        <ConfirmProvider>
          <BorrowPanel backend={slowBackend} />
        </ConfirmProvider>
      </MemoryRouter>,
    );
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
      target: { value: "hi" },
    });
    fireEvent.submit(submitOf(t("llmShare.borrow.submit")));
    await screen.findByRole("alertdialog");
    fireEvent.click(screen.getByRole("button", { name: t("common.actions.confirm") }));
    expect(await screen.findByTestId("borrow-calling")).toBeTruthy();
    resolveBorrow({
      status: "done",
      receipt: { reqId: "req-1", appended: true, estimated: false, disputeWindowSecs: 86400 },
      sseCount: 1,
    });
    await waitFor(() => expect(screen.queryByTestId("borrow-calling")).toBeNull());
    expect(await screen.findByTestId("borrow-report")).toBeTruthy();
  });
});

describe("R2-15 留存自述字段渲染", () => {
  it("渲染可选输入与用途说明，值随发布请求提交", async () => {
    const { backend } = makeLlmShareMockPair({ now: () => 1788549300 });
    const spy = vi.spyOn(backend, "offerPublish");
    render(<OfferPanel backend={backend} />);
    // UX：表单默认收起，留存自述归入高级折叠组
    await screen.findByTestId("offer-publish-cta");
    fireEvent.click(screen.getByTestId("offer-publish-cta"));
    fireEvent.change(screen.getByLabelText(t("llmShare.offer.formModels")), {
      target: { value: "gpt-4o" },
    });
    fireEvent.change(screen.getByLabelText("gpt-4o"), {
      target: { value: "10" },
    });
    fireEvent.change(screen.getByLabelText(t("llmShare.offer.formPeriodEnds")), {
      target: { value: "2026-09-30" },
    });
    fireEvent.click(screen.getByTestId("offer-advanced-toggle"));
    fireEvent.change(screen.getByLabelText(t("llmShare.offer.formRetention")), {
      target: { value: "ephemeral-30d" },
    });
    expect(
      document.getElementById("llm-offer-retention-hint")?.textContent,
    ).toContain(t("llmShare.offer.formRetentionHint"));
    fireEvent.submit(submitOf(t("llmShare.offer.publish")));
    await screen.findByTestId("offer-status");
    expect(spy.mock.calls[0][0].retention).toBe("ephemeral-30d");
  });
});