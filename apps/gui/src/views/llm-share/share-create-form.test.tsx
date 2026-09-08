import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import "@/i18n";
import i18n from "@/i18n";

import { makeLlmShareMockPair } from "./mock-backend";
import type { ProviderConfig } from "./provider-configs";
import { ShareCreateForm } from "./share-create-form";

const t = i18n.t.bind(i18n);

const CONFIG: ProviderConfig = {
  id: "pv-1",
  name: "DeepSeek 官方",
  baseUrl: "https://api.deepseek.com/v1",
  protocol: "openai",
  apiKey: "",
  apiKeyMasked: "sk-••••abcd",
  models: ["deepseek-chat", "deepseek-reasoner"],
  createdAt: 1,
};

const { toastSuccessMock, toastErrorMock } = vi.hoisted(() => ({
  toastSuccessMock: vi.fn(),
  toastErrorMock: vi.fn(),
}));
vi.mock("@/components/feedback/toast", () => ({
  toastSuccess: toastSuccessMock,
  toastError: toastErrorMock,
}));

function renderForm(backend: Parameters<typeof ShareCreateForm>[0]["backend"]) {
  return render(
    <MemoryRouter>
      <ShareCreateForm provider={CONFIG} backend={backend} onCreated={() => {}} onCancel={() => {}} />
    </MemoryRouter>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("share-create-form：生成 dsh-llm-share:// 分享链接（契约 §16.6）", () => {
  it("模型预填 provider 全量；提交 shareCreate → 展示链接与复制/发送到聊天", async () => {
    const { backend, mock } = makeLlmShareMockPair({ now: () => 1788549300 });
    mock.providerSave({
      id: CONFIG.id,
      name: CONFIG.name,
      baseUrl: CONFIG.baseUrl,
      protocol: CONFIG.protocol,
      apiKey: "sk-test-1234567890abcd",
      models: CONFIG.models,
    });
    mock.offerPublish({
      models: CONFIG.models,
      spare: { "deepseek-chat": 100, "deepseek-reasoner": 100 },
      periodEnds: "2026-09-30",
    });
    renderForm(backend);
    // 模型勾选预填全部
    const gptCheck = screen.getByTestId("share-model-deepseek-chat").querySelector("input") as HTMLInputElement;
    expect(gptCheck.checked).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.share.submit") }));
    const result = await screen.findByTestId("share-create-result");
    const linkInput = result.querySelector("input") as HTMLInputElement;
    expect(linkInput.value).toContain("dsh-llm-share://v1?");
    expect(linkInput.value).toContain("peer=");
    expect(screen.getByTestId("share-link-copy")).toBeTruthy();
    expect(screen.getByTestId("share-link-send-chat")).toBeTruthy();
    expect(mock.shareList().shares).toHaveLength(1);
    expect(mock.shareList().shares[0].models).toEqual(CONFIG.models);
  });

  it("取消勾选全部模型：提交拦截并提示，后端零调用", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    mock.providerSave({
      id: CONFIG.id,
      name: CONFIG.name,
      baseUrl: CONFIG.baseUrl,
      protocol: CONFIG.protocol,
      apiKey: "sk-test-1234567890abcd",
      models: CONFIG.models,
    });
    mock.offerPublish({
      models: CONFIG.models,
      spare: { "deepseek-chat": 100, "deepseek-reasoner": 100 },
      periodEnds: "2026-09-30",
    });
    renderForm(backend);
    for (const model of CONFIG.models) {
      fireEvent.click(screen.getByTestId("share-model-" + model));
    }
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.share.submit") }));
    expect(await screen.findByTestId("share-action-error")).toBeTruthy();
    expect(mock.shareList().shares).toHaveLength(0);
  });

  it("未发布 offer：显式提示，提交不可用（常态非故障）", async () => {
    const { backend } = makeLlmShareMockPair();
    renderForm(backend);
    expect(await screen.findByTestId("share-offer-missing")).toBeTruthy();
    expect(screen.getByRole("button", { name: t("llmShare.share.submit") })).toBeDisabled();
  });

  it("复制按钮走 navigator.clipboard 并 toast", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    const { backend, mock } = makeLlmShareMockPair();
    mock.providerSave({
      id: CONFIG.id,
      name: CONFIG.name,
      baseUrl: CONFIG.baseUrl,
      protocol: CONFIG.protocol,
      apiKey: "sk-test-1234567890abcd",
      models: CONFIG.models,
    });
    mock.offerPublish({
      models: CONFIG.models,
      spare: { "deepseek-chat": 100, "deepseek-reasoner": 100 },
      periodEnds: "2026-09-30",
    });
    renderForm(backend);
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.share.submit") }));
    const result = await screen.findByTestId("share-create-result");
    const link = (result.querySelector("input") as HTMLInputElement).value;
    fireEvent.click(screen.getByTestId("share-link-copy"));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith(link));
    expect(toastSuccessMock).toHaveBeenCalled();
  });

  it("发送到聊天：跳 /chat?compose= 预填输入框", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    mock.providerSave({
      id: CONFIG.id,
      name: CONFIG.name,
      baseUrl: CONFIG.baseUrl,
      protocol: CONFIG.protocol,
      apiKey: "sk-test-1234567890abcd",
      models: CONFIG.models,
    });
    mock.offerPublish({
      models: CONFIG.models,
      spare: { "deepseek-chat": 100, "deepseek-reasoner": 100 },
      periodEnds: "2026-09-30",
    });
    const { container } = render(
      <MemoryRouter initialEntries={["/llm-share"]}>
        <ConfirmProvider>
          <ShareCreateForm provider={CONFIG} backend={backend} onCreated={() => {}} onCancel={() => {}} />
        </ConfirmProvider>
      </MemoryRouter>,
    );
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.share.submit") }));
    const result = await screen.findByTestId("share-create-result");
    const link = (result.querySelector("input") as HTMLInputElement).value;
    fireEvent.click(screen.getByTestId("share-link-send-chat"));
    await waitFor(() => expect(toastSuccessMock).toHaveBeenCalledWith(t("llmShare.share.sentToChat")));
    expect(link).toContain("dsh-llm-share://");
    void container;
  });
});