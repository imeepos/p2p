import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { MemoryRouter } from "react-router-dom";

import "@/i18n";
import i18n from "@/i18n";

import { resetBorrowPrefill } from "./borrow-prefill";
import { makeLlmShareMockPair } from "./mock-backend";
import { LlmShareView } from "./llm-share-view";

const t = i18n.t.bind(i18n);
const PEER = "52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd";

const TAB_LABELS = [
  "llmShare.tabs.overview",
  "llmShare.tabs.borrow",
  "llmShare.tabs.ledger",
  "llmShare.tabs.offer",
  "llmShare.tabs.allowlist",
  "llmShare.tabs.providers",
] as const;

afterEach(() => {
  cleanup();
  resetBorrowPrefill();
});

describe("/llm-share tab 化（概览统计首页 + 列表/配置 tab）", () => {
  it("六个 tab 齐备；默认落概览 tab（统计分析），借用表单不出现", async () => {
    const { backend } = makeLlmShareMockPair();
    render(
      <MemoryRouter>
        <LlmShareView backend={backend} />
      </MemoryRouter>,
    );
    expect(screen.getByRole("heading", { level: 1, name: t("llmShare.title") })).toBeTruthy();
    for (const key of TAB_LABELS) {
      expect(screen.getByRole("tab", { name: t(key) })).toBeTruthy();
    }
    expect(await screen.findByTestId("overview-panel")).toBeTruthy();
    expect(screen.queryByLabelText(t("llmShare.borrow.formTargetPeer"))).toBeNull();
  });

  it("概览统计卡与快捷动作、资源计数渲染（空数据为 0 值态）", async () => {
    const { backend } = makeLlmShareMockPair();
    render(
      <MemoryRouter>
        <LlmShareView backend={backend} />
      </MemoryRouter>,
    );
    expect((await screen.findByTestId("stat-entries-value")).textContent).toBe("0");
    expect(screen.getByTestId("stat-tokens-value").textContent).toBe("0");
    expect(screen.getByTestId("overview-go-borrow").textContent).toContain(t("llmShare.overview.goBorrow"));
    expect(screen.getByTestId("overview-allow").textContent).toContain(
      t("llmShare.overview.allowlistValue", { count: 0 }),
    );
    expect(screen.getByText(t("llmShare.overview.recentTitle"))).toBeTruthy();
    expect(screen.getByText(t("llmShare.ledger.emptyList"))).toBeTruthy();
  });

  it("有数据时概览统计非零且最近流水出现", async () => {
    const { backend, mock } = makeLlmShareMockPair({ now: () => 1788549300 });
    mock.offerPublish({ models: ["gpt-4o"], spare: { "gpt-4o": 7 }, periodEnds: "2026-09-30" });
    mock.allow({ peerId: PEER });
    mock.borrow({
      targetPeer: PEER,
      model: "gpt-4o",
      messages: "hi",
      maxTokens: 8,
      reqId: "R1",
    });
    render(
      <MemoryRouter>
        <LlmShareView backend={backend} />
      </MemoryRouter>,
    );
    expect((await screen.findByTestId("stat-entries-value")).textContent).toBe("1");
    expect(screen.getByTestId("overview-recent-row")).toBeTruthy();
    expect(screen.getByTestId("overview-offer").textContent).toContain("gpt-4o");
  });

  it("点借用 tab：借用表单出现，概览卸载（一次只看一屏）", async () => {
    const { backend } = makeLlmShareMockPair();
    render(
      <MemoryRouter>
        <LlmShareView backend={backend} />
      </MemoryRouter>,
    );
    await screen.findByTestId("overview-panel");
    // Radix TabsTrigger 在 mousedown 激活（fireEvent.click 单发不触发）
    fireEvent.mouseDown(screen.getByRole("tab", { name: t("llmShare.tabs.borrow") }));
    expect(await screen.findByLabelText(t("llmShare.borrow.formTargetPeer"))).toBeTruthy();
    expect(screen.queryByTestId("overview-panel")).toBeNull();
  });

  it("深链 ?tab=offer 直接落能力发布 tab", async () => {
    const { backend } = makeLlmShareMockPair();
    render(
      <MemoryRouter initialEntries={["/llm-share?tab=offer"]}>
        <LlmShareView backend={backend} />
      </MemoryRouter>,
    );
    expect(await screen.findByTestId("offer-panel")).toBeTruthy();
    expect(screen.queryByTestId("overview-panel")).toBeNull();
  });

  it("非激活 tab 不挂载：默认落点下账本/白名单面板不出现，切过去才渲染", async () => {
    const { backend } = makeLlmShareMockPair();
    render(
      <MemoryRouter>
        <LlmShareView backend={backend} />
      </MemoryRouter>,
    );
    await screen.findByTestId("overview-panel");
    expect(screen.queryByTestId("ledger-panel")).toBeNull();
    expect(screen.queryByTestId("allowlist-panel")).toBeNull();
    fireEvent.mouseDown(screen.getByRole("tab", { name: t("llmShare.tabs.ledger") }));
    expect(await screen.findByTestId("ledger-panel")).toBeTruthy();
  });
});
