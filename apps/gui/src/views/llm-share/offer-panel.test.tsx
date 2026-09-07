import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import "@/i18n";
import i18n from "@/i18n";

import { makeLlmShareMockPair } from "./mock-backend";
import { resetOfferLoadWarnForTest } from "./offer-errors";
import { OfferPanel } from "./offer-panel";
import type { LlmOfferStatus, LlmShareBackend } from "./types";

const t = i18n.t.bind(i18n);

// R2-08：发布成功 toast 反馈（toast 模块整体 mock，专断言调用）
const { toastSuccessMock, toastErrorMock } = vi.hoisted(() => ({
  toastSuccessMock: vi.fn(),
  toastErrorMock: vi.fn(),
}));
vi.mock("@/components/feedback/toast", () => ({
  toastSuccess: toastSuccessMock,
  toastError: toastErrorMock,
}));

function rejectAll(): LlmShareBackend {
  const fail = () => Promise.reject(new Error("boom"));
  return {
    offerPublish: fail,
    offerShow: fail,
    allowList: fail,
    allow: fail,
    deny: fail,
    borrow: fail,
    ledgerList: fail,
    ledgerBalance: fail,
    receiptVerify: fail,
    providerList: fail,
    providerSave: fail,
    providerRemove: fail,
    shareCreate: fail,
    shareList: fail,
    shareRevoke: fail,
    shareRedeem: fail,
    serveStatus: fail,
  };
}

// UX：表单默认收起，走查需先点「发布能力声明」展开（CTA 随加载异步出现）
async function openForm() {
  fireEvent.click(await screen.findByTestId("offer-publish-cta"));
}

function submitPublish() {
  const button = screen.getByRole("button", { name: t("llmShare.offer.publish") });
  const form = button.closest("form");
  if (!form) throw new Error("test precondition broken: publish form missing");
  fireEvent.submit(form);
}

function fillValidForm() {
  fireEvent.change(screen.getByLabelText(t("llmShare.offer.formModels")), {
    target: { value: "gpt-4o,deepseek-v3" },
  });
  fireEvent.change(screen.getByLabelText("gpt-4o"), {
    target: { value: "1500000" },
  });
  fireEvent.change(screen.getByLabelText("deepseek-v3"), {
    target: { value: "999999999" },
  });
  fireEvent.change(screen.getByLabelText(t("llmShare.offer.formPeriodEnds")), {
    target: { value: "2026-09-30" },
  });
}

afterEach(() => cleanup());

describe("LLM3 offer 面板（契约 §16.1 必填集 / §16.2-5 五态两级）", () => {
  it("首屏信息优先：未发布时空态 CTA，表单点按钮才展开", async () => {
    resetOfferLoadWarnForTest();
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { backend } = makeLlmShareMockPair();
    render(<OfferPanel backend={backend} />);
    await screen.findByText(t("llmShare.offer.emptyTitle"));
    expect(screen.queryByTestId("offer-publish-form")).toBeNull();
    fireEvent.click(screen.getByTestId("offer-publish-cta"));
    expect(await screen.findByTestId("offer-publish-form")).toBeTruthy();
    expect(warnSpy).not.toHaveBeenCalled();
    warnSpy.mockRestore();
  });

  it("已发布时首屏是状态卡，更新表单点「更新声明」展开并回填", async () => {
    const { mock, backend } = makeLlmShareMockPair({ now: () => 1788549300 });
    mock.offerPublish({ models: ["gpt-4o"], spare: { "gpt-4o": 7 }, periodEnds: "2026-09-30" });
    render(<OfferPanel backend={backend} />);
    await screen.findByTestId("offer-status");
    expect(screen.queryByTestId("offer-publish-form")).toBeNull();
    fireEvent.click(screen.getByTestId("offer-update"));
    await screen.findByTestId("offer-publish-form");
    const models = screen.getByLabelText(t("llmShare.offer.formModels")) as HTMLInputElement;
    expect(models.value).toBe("gpt-4o");
    const spare = screen.getByLabelText("gpt-4o") as HTMLInputElement;
    expect(spare.value).toBe("7");
  });

  it("空表单提交被必填集拦截且不触达后端（契约显性化）", async () => {
    const { mock, backend } = makeLlmShareMockPair();
    const spy = vi.spyOn(backend, "offerPublish");
    render(<OfferPanel backend={backend} />);
    await openForm();
    submitPublish();
    const alerts = await screen.findAllByRole("alert");
    expect(alerts.map((a) => a.textContent).join(" ")).toContain(
      t("llmShare.offer.errModelsRequired"),
    );
    expect(spy).not.toHaveBeenCalled();
    expect(mock.allowList()).toBeTruthy();
  });

  it("闲量缺填与非法值分别给字段级错误（结构化行从源头杜绝漏行）", async () => {
    const { backend } = makeLlmShareMockPair();
    render(<OfferPanel backend={backend} />);
    await openForm();
    fireEvent.change(screen.getByLabelText(t("llmShare.offer.formModels")), {
      target: { value: "gpt-4o,m2" },
    });
    submitPublish();
    const missing = await screen.findAllByRole("alert");
    expect(missing.map((a) => a.textContent).join(" ")).toContain(
      t("llmShare.offer.errSpareRequired"),
    );
    fireEvent.change(screen.getByLabelText("gpt-4o"), {
      target: { value: "10" },
    });
    fireEvent.change(screen.getByLabelText("m2"), {
      target: { value: "0" },
    });
    submitPublish();
    const invalid = await screen.findAllByRole("alert");
    expect(invalid.map((a) => a.textContent).join(" ")).toContain(
      t("llmShare.offer.errPositiveInt"),
    );
  });

  it("模型清单快捷带入：上游配置模型一键追加", async () => {
    const { backend } = makeLlmShareMockPair();
    localStorage.setItem(
      "p2p-gui-llm-providers",
      JSON.stringify([
        {
          id: "pv-1",
          name: "DeepSeek",
          baseUrl: "https://api.deepseek.com/v1",
          apiKey: "sk-test-123456",
          models: ["deepseek-v3"],
          createdAt: 1,
        },
      ]),
    );
    render(<OfferPanel backend={backend} />);
    await openForm();
    fireEvent.click(await screen.findByTestId("offer-quickadd-deepseek-v3"));
    const models = screen.getByLabelText(t("llmShare.offer.formModels")) as HTMLInputElement;
    expect(models.value).toBe("deepseek-v3");
    localStorage.clear();
  });

  it("账期快捷预设一键填 +30 天", async () => {
    const { backend } = makeLlmShareMockPair();
    render(<OfferPanel backend={backend} />);
    await openForm();
    fireEvent.click(screen.getByTestId("offer-period-preset-30d"));
    const period = screen.getByLabelText(t("llmShare.offer.formPeriodEnds")) as HTMLInputElement;
    expect(period.value).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it("发布成功渲染 live 态与五字段，表单收起", async () => {
    const { backend } = makeLlmShareMockPair({ now: () => 1788549300 });
    render(<OfferPanel backend={backend} />);
    await openForm();
    fillValidForm();
    submitPublish();
    const status = await screen.findByTestId("offer-status");
    expect(status.getAttribute("data-status")).toBe("live");
    expect(status.textContent).toContain("gpt-4o");
    expect(status.textContent).toContain("2026-09-30");
    expect(await screen.findByTestId("offer-update")).toBeTruthy();
    expect(screen.queryByTestId("offer-publish-form")).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it.each<LlmOfferStatus>(["expired", "not_yet_valid"])(
    "常态中性态 %s：无警示 role，不渲染失败样式",
    async (status) => {
      const { mock, backend } = makeLlmShareMockPair({ forceOfferStatus: status });
      mock.offerPublish({ models: ["gpt-4o"], spare: { "gpt-4o": 5 }, periodEnds: "2026-09-30" });
      const view = render(<OfferPanel backend={backend} />);
      await vi.waitFor(() => {
        expect(view.container.querySelector('[data-testid="offer-status"]')).not.toBeNull();
      });
      const card = view.container.querySelector('[data-testid="offer-status"]');
      expect(card?.getAttribute("data-status")).toBe(status);
      expect(screen.queryByRole("alert")).toBeNull();
    },
  );

  it.each<LlmOfferStatus>(["peer_mismatch", "bad_signature"])(
    "醒目警示态 %s：danger 徽章 + role=alert 安全提示",
    async (status) => {
      const { mock, backend } = makeLlmShareMockPair({ forceOfferStatus: status });
      mock.offerPublish({ models: ["gpt-4o"], spare: { "gpt-4o": 5 }, periodEnds: "2026-09-30" });
      const view = render(<OfferPanel backend={backend} />);
      await vi.waitFor(() => {
        expect(view.container.querySelector('[data-testid="offer-status"]')).not.toBeNull();
      });
      const card = view.container.querySelector('[data-testid="offer-status"]');
      expect(card?.getAttribute("data-status")).toBe(status);
      expect(card?.className).toContain("border-destructive");
      expect(screen.getByRole("alert")).toHaveTextContent(t("llmShare.offer.securityWarning"));
    },
  );

  it("发布失败路径显式可观测（错误原样露出不吞）", async () => {
    render(<OfferPanel backend={rejectAll()} />);
    await openForm();
    fillValidForm();
    submitPublish();
    expect(await screen.findByRole("alert")).toHaveTextContent("boom");
    expect(toastErrorMock).toHaveBeenCalled();
  });

  // R2-03 回归：未发布是常态非故障，空态走中文出路文案，不露内部英文报错
  it("未发布空态走中文出路文案，不显示内部报错原文，console 静默", async () => {
    resetOfferLoadWarnForTest();
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { backend } = makeLlmShareMockPair();
    render(<OfferPanel backend={backend} />);
    expect(await screen.findByText(t("llmShare.offer.emptyTitle"))).toBeTruthy();
    expect(await screen.findByText(t("llmShare.offer.emptyHint"))).toBeTruthy();
    expect(screen.queryByText(/never published/u)).toBeNull();
    expect(warnSpy).not.toHaveBeenCalled();
    warnSpy.mockRestore();
  });

  // R2-26 回归：真实错误整个会话仅 console 提示一次，但错误原文始终可见
  it("真实错误显示错误原文，console 告警会话级单次", async () => {
    resetOfferLoadWarnForTest();
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    render(<OfferPanel backend={rejectAll()} />);
    expect((await screen.findAllByText("boom"))[0]).toBeTruthy();
    const second = render(<OfferPanel backend={rejectAll()} />);
    expect((await screen.findAllByText("boom"))[0]).toBeTruthy();
    expect(warnSpy).toHaveBeenCalledTimes(1);
    second.unmount();
    warnSpy.mockRestore();
  });

  // R2-08 回归：发布成功补 toast 确认，不再仅状态卡静默出现
  it("发布成功触发成功 toast", async () => {
    const { backend } = makeLlmShareMockPair({ now: () => 1788549300 });
    render(<OfferPanel backend={backend} />);
    await openForm();
    fillValidForm();
    submitPublish();
    await screen.findByTestId("offer-status");
    expect(toastSuccessMock).toHaveBeenCalledWith(t("llmShare.offer.publishSuccess"));
  });

  // R2-09 回归：剩余时间人性化——live 显时长、过期显「已过期」语义
  it("剩余时间显人性化时长而非裸秒", async () => {
    const { backend, mock } = makeLlmShareMockPair({ now: () => 1788549300 });
    mock.offerPublish({ models: ["gpt-4o"], spare: { "gpt-4o": 5 }, periodEnds: "2026-09-30" });
    const view = render(<OfferPanel backend={backend} />);
    const remaining = await vi.waitFor(() => {
      const el = view.container.querySelector('[data-testid="offer-remaining"]');
      if (!el) throw new Error("remaining cell not rendered yet");
      return el;
    });
    expect(remaining.textContent).toMatch(/小时|分|秒/u);
    expect(remaining.textContent).not.toBe("3600");
  });

  it.each<LlmOfferStatus>(["expired", "not_yet_valid"])(
    "%s 态剩余时间给状态语义而非 0 秒",
    async (status) => {
      const { backend, mock } = makeLlmShareMockPair({ forceOfferStatus: status });
      mock.offerPublish({ models: ["gpt-4o"], spare: { "gpt-4o": 5 }, periodEnds: "2026-09-30" });
      const view = render(<OfferPanel backend={backend} />);
      const remaining = await vi.waitFor(() => {
        const el = view.container.querySelector('[data-testid="offer-remaining"]');
        if (!el) throw new Error("remaining cell not rendered yet");
        return el;
      });
      expect(remaining.textContent?.length).toBeGreaterThan(0);
      expect(remaining.textContent).not.toBe("0");
    },
  );
});