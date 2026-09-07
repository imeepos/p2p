import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import "@/i18n";
import i18n from "@/i18n";

import { makeLlmShareMockPair } from "./mock-backend";
import { maskApiKey, saveProviderConfigs, type ProviderConfig } from "./provider-configs";
import { ProviderPanel } from "./provider-panel";

const t = i18n.t.bind(i18n);
const FRIEND = "52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd";
const API_KEY = "sk-test-1234567890abcd";

const CONFIG: ProviderConfig = {
  id: "pv-1",
  name: "DeepSeek 官方",
  baseUrl: "https://api.deepseek.com/v1",
  apiKey: API_KEY,
  models: ["deepseek-chat", "deepseek-reasoner"],
  createdAt: 1,
};

function renderPanel(backend: Parameters<typeof ProviderPanel>[0]["backend"]) {
  return render(
    <ConfirmProvider>
      <ProviderPanel backend={backend} />
    </ConfirmProvider>,
  );
}

function fillField(label: string, value: string) {
  fireEvent.change(screen.getByLabelText(label), { target: { value } });
}

afterEach(() => {
  cleanup();
  localStorage.clear();
});

describe("上游配置面板：本地自用列表", () => {
  it("空态渲染引导文案", async () => {
    const { backend } = makeLlmShareMockPair();
    renderPanel(backend);
    expect(await screen.findByText(t("llmShare.providers.emptyTitle"))).toBeTruthy();
  });

  it("新增配置：保存后入列，密钥只显示掩码并写穿 localStorage", async () => {
    const { backend } = makeLlmShareMockPair();
    renderPanel(backend);
    fireEvent.click(screen.getByTestId("provider-add"));
    fillField(t("llmShare.providers.formName"), "DeepSeek 官方");
    fillField(t("llmShare.providers.formBaseUrl"), "https://api.deepseek.com/v1");
    fillField(t("llmShare.providers.formApiKey"), API_KEY);
    fillField(t("llmShare.providers.formModels"), "deepseek-chat, deepseek-reasoner");
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.providers.save") }));
    const row = await screen.findByTestId("provider-row");
    expect(row.textContent).toContain("DeepSeek 官方");
    expect(row.textContent).toContain(maskApiKey(API_KEY));
    expect(row.textContent).not.toContain(API_KEY);
    expect(JSON.parse(localStorage.getItem("p2p-gui-llm-providers") ?? "[]")).toHaveLength(1);
  });

  it("必填缺失：逐字段报错且不入列不落盘", async () => {
    const { backend } = makeLlmShareMockPair();
    renderPanel(backend);
    fireEvent.click(screen.getByTestId("provider-add"));
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.providers.save") }));
    expect(await screen.findByText(t("llmShare.providers.errNameRequired"))).toBeTruthy();
    expect(screen.queryByTestId("provider-row")).toBeNull();
    expect(localStorage.getItem("p2p-gui-llm-providers")).toBeNull();
  });

  it("删除经破坏性二次确认：确认后列表与存档同步移除", async () => {
    saveProviderConfigs([CONFIG]);
    const { backend } = makeLlmShareMockPair();
    renderPanel(backend);
    const row = await screen.findByTestId("provider-row");
    fireEvent.click(within(row).getByRole("button", { name: t("llmShare.providers.remove") }));
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain(t("llmShare.providers.removeConfirmTitle"));
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.providers.remove") }));
    expect(await screen.findByText(t("llmShare.providers.emptyTitle"))).toBeTruthy();
    expect(JSON.parse(localStorage.getItem("p2p-gui-llm-providers") ?? "[]")).toHaveLength(0);
  });
});

describe("分享给好友：offerPublish + allow 双动作，密钥不出本机", () => {
  it("按配置模型发布声明并把好友按同批模型放行，请求体不含 apiKey", async () => {
    saveProviderConfigs([CONFIG]);
    const { backend, mock } = makeLlmShareMockPair();
    const publishSpy = vi.spyOn(backend, "offerPublish");
    const allowSpy = vi.spyOn(backend, "allow");
    renderPanel(backend);
    const row = await screen.findByTestId("provider-row");
    fireEvent.click(within(row).getByTestId("provider-share-toggle"));
    await screen.findByTestId("provider-share-form");
    // 闲量按模型预填 model= 行，数值由用户填写（不替用户编造额度）
    expect(screen.getByLabelText(t("llmShare.providers.shareSpare")).textContent).toBe(
      "deepseek-chat=\ndeepseek-reasoner=",
    );
    fillField(t("llmShare.providers.shareSpare"), "deepseek-chat=1500000\ndeepseek-reasoner=900000");
    fillField(t("llmShare.providers.sharePeerId"), FRIEND);
    fillField(t("llmShare.providers.sharePeriodEnds"), "2026-09-30");
    fillField(t("llmShare.providers.shareNote"), "首月互借");
    fireEvent.click(
      screen.getByRole("button", { name: t("llmShare.providers.shareSubmit") }),
    );
    await waitFor(() => expect(allowSpy).toHaveBeenCalled());
    expect(publishSpy.mock.calls[0][0].models).toEqual(CONFIG.models);
    expect(allowSpy.mock.calls[0][0].peerId).toBe(FRIEND);
    expect(allowSpy.mock.calls[0][0].models).toEqual(CONFIG.models);
    expect(JSON.stringify(publishSpy.mock.calls)).not.toContain(API_KEY);
    expect(JSON.stringify(allowSpy.mock.calls)).not.toContain(API_KEY);
    expect(mock.allowList().entries.map((e) => e.peerId)).toEqual([FRIEND]);
    expect(mock.offerShow().models).toEqual(CONFIG.models);
    // 分享成功后分享表单收起，回列表态
    await waitFor(() => expect(screen.queryByTestId("provider-share-form")).toBeNull());
  });

  it("校验失败不触发任何后端调用（默认拒绝语义）", async () => {
    saveProviderConfigs([CONFIG]);
    const { backend, mock } = makeLlmShareMockPair();
    const publishSpy = vi.spyOn(backend, "offerPublish");
    renderPanel(backend);
    const row = await screen.findByTestId("provider-row");
    fireEvent.click(within(row).getByTestId("provider-share-toggle"));
    await screen.findByTestId("provider-share-form");
    // 闲量留预填空值 + 不填好友与账期，直接提交
    fireEvent.click(
      screen.getByRole("button", { name: t("llmShare.providers.shareSubmit") }),
    );
    const alerts = await screen.findAllByRole("alert");
    expect(alerts.length).toBeGreaterThanOrEqual(2);
    expect(publishSpy).not.toHaveBeenCalled();
    expect(mock.allowList().entries).toHaveLength(0);
  });
});
