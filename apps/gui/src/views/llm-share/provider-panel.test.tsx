import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import "@/i18n";
import i18n from "@/i18n";

import { makeLlmShareMockPair } from "./mock-backend";
import { maskApiKey, PROVIDER_STORAGE_KEY, type ProviderConfig } from "./provider-configs";
import { ProviderPanel } from "./provider-panel";

const t = i18n.t.bind(i18n);
const API_KEY = "sk-test-1234567890abcd";

const CONFIG: ProviderConfig = {
  id: "pv-1",
  name: "DeepSeek 官方",
  baseUrl: "https://api.deepseek.com/v1",
  protocol: "openai",
  apiKey: API_KEY,
  models: ["deepseek-chat", "deepseek-reasoner"],
  createdAt: 1,
};

function renderPanel(backend: Parameters<typeof ProviderPanel>[0]["backend"]) {
  return render(
    <MemoryRouter>
      <ConfirmProvider>
        <ProviderPanel backend={backend} />
      </ConfirmProvider>
    </MemoryRouter>,
  );
}

function fillField(label: string, value: string) {
  fireEvent.change(screen.getByLabelText(label), { target: { value } });
}

afterEach(() => {
  cleanup();
  localStorage.clear();
});

describe("上游配置面板：后端 ProviderStore 驱动（契约 §16.6）", () => {
  it("空态渲染引导文案与 serve 装配卡（缺省未装配）", async () => {
    const { backend } = makeLlmShareMockPair();
    renderPanel(backend);
    expect(await screen.findByText(t("llmShare.providers.emptyTitle"))).toBeTruthy();
    expect(await screen.findByTestId("serve-status-card")).toBeTruthy();
    expect(await screen.findByTestId("serve-not-assembled")).toBeTruthy();
  });

  it("新增配置：协议 radio 缺省 openai 可选 claude，保存后入列且只显示掩码，localStorage 不落明文键", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    renderPanel(backend);
    fireEvent.click(screen.getByTestId("provider-add"));
    fillField(t("llmShare.providers.formName"), "DeepSeek 官方");
    fillField(t("llmShare.providers.formBaseUrl"), "https://api.deepseek.com/v1");
    fillField(t("llmShare.providers.formApiKey"), API_KEY);
    fillField(t("llmShare.providers.formModels"), "deepseek-chat, deepseek-reasoner");
    // 协议 radio：缺省 openai；切 claude 再切回 openai
    expect((screen.getByTestId("provider-protocol-openai").querySelector("input") as HTMLInputElement).checked).toBe(true);
    fireEvent.click(screen.getByTestId("provider-protocol-claude"));
    expect((screen.getByTestId("provider-protocol-claude").querySelector("input") as HTMLInputElement).checked).toBe(true);
    fireEvent.click(screen.getByTestId("provider-protocol-openai"));
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.providers.save") }));
    const row = await screen.findByTestId("provider-row");
    expect(row.textContent).toContain("DeepSeek 官方");
    expect(row.textContent).toContain(maskApiKey(API_KEY));
    expect(row.textContent).not.toContain(API_KEY);
    expect(mock.providerList().providers).toHaveLength(1);
    expect(mock.providerList().providers[0].protocol).toBe("openai");
    expect(localStorage.getItem(PROVIDER_STORAGE_KEY)).toBeNull();
  });

  it("http:// baseUrl 显式告警", async () => {
    const { backend } = makeLlmShareMockPair();
    renderPanel(backend);
    fireEvent.click(screen.getByTestId("provider-add"));
    fillField(t("llmShare.providers.formBaseUrl"), "http://api.example.com/v1");
    expect(await screen.findByTestId("provider-http-warning")).toBeTruthy();
  });

  it("必填缺失：逐字段报错且不入列不写后端", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    renderPanel(backend);
    fireEvent.click(screen.getByTestId("provider-add"));
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.providers.save") }));
    expect(await screen.findByText(t("llmShare.providers.errNameRequired"))).toBeTruthy();
    expect(screen.queryByTestId("provider-row")).toBeNull();
    expect(mock.providerList().providers).toHaveLength(0);
  });

  it("删除经破坏性二次确认：确认后列表与后端同步移除", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    mock.providerSave({
      id: CONFIG.id,
      name: CONFIG.name,
      baseUrl: CONFIG.baseUrl,
      protocol: CONFIG.protocol,
      apiKey: CONFIG.apiKey,
      models: CONFIG.models,
    });
    renderPanel(backend);
    const row = await screen.findByTestId("provider-row");
    fireEvent.click(within(row).getByRole("button", { name: t("llmShare.providers.remove") }));
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain(t("llmShare.providers.removeConfirmTitle"));
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.providers.remove") }));
    expect(await screen.findByText(t("llmShare.providers.emptyTitle"))).toBeTruthy();
    expect(mock.providerList().providers).toHaveLength(0);
  });

  it("serve 装配态：assembled=true 展示就绪，lastError 显式告警", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    mock.setServeStatus({ assembled: true, models: ["gpt-4o"], lastError: undefined });
    renderPanel(backend);
    expect(await screen.findByTestId("serve-assembled")).toBeTruthy();
    mock.setServeStatus({ assembled: false, models: [], lastError: "provider missing" });
    fireEvent.click(screen.getByTestId("serve-status-refresh"));
    expect(await screen.findByTestId("serve-not-assembled")).toBeTruthy();
    expect(await screen.findByTestId("serve-last-error")).toBeTruthy();
  });
});

describe("localStorage 一次性幂等迁移（§16.6-1）", () => {
  it("挂载时旧键条目迁入后端并 removeItem；迁移后写路径不再落明文键", async () => {
    localStorage.setItem(
      PROVIDER_STORAGE_KEY,
      JSON.stringify([
        { id: "pv-1", name: "旧配置", baseUrl: "https://old/v1", apiKey: API_KEY, models: ["m1"], createdAt: 1 },
      ]),
    );
    const { backend, mock } = makeLlmShareMockPair();
    renderPanel(backend);
    const row = await screen.findByTestId("provider-row");
    expect(row.textContent).toContain("旧配置");
    expect(localStorage.getItem(PROVIDER_STORAGE_KEY)).toBeNull();
    expect(mock.providerList().providers).toHaveLength(1);
    expect(mock.providerList().providers[0].protocol).toBe("openai");
  });
});

describe("生成分享链接入口（v13 取代手动分享表单）", () => {
  it("行内入口展开 share-create 表单；未发布 offer 显式提示", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    mock.providerSave({
      id: CONFIG.id,
      name: CONFIG.name,
      baseUrl: CONFIG.baseUrl,
      protocol: CONFIG.protocol,
      apiKey: CONFIG.apiKey,
      models: CONFIG.models,
    });
    renderPanel(backend);
    const row = await screen.findByTestId("provider-row");
    fireEvent.click(within(row).getByTestId("provider-create-share"));
    expect(await screen.findByTestId("share-create-form")).toBeTruthy();
    // 未发布 offer：模型 ⊆ 校验受限，显式提示（常态非故障）
    expect(await screen.findByTestId("share-offer-missing")).toBeTruthy();
  });
});