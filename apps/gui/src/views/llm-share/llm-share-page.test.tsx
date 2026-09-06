import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import "@/i18n";
import i18n from "@/i18n";

import { makeLlmShareMockPair } from "./mock-backend";
import { LlmShareView } from "./llm-share-view";

const t = i18n.t.bind(i18n);

afterEach(() => cleanup());

describe("LLM3 /llm-share 四面板（契约 §16.3 落点）", () => {
  it("四面板齐备：offer/allowlist/borrow/ledger 四 section 与页头", async () => {
    const { backend } = makeLlmShareMockPair();
    render(<LlmShareView backend={backend} />);
    expect(screen.getByRole("heading", { level: 1, name: t("llmShare.title") })).toBeTruthy();
    for (const [testid, key] of [
      ["section-offer", "llmShare.panels.offer"],
      ["section-allowlist", "llmShare.panels.allowlist"],
      ["section-borrow", "llmShare.panels.borrow"],
      ["section-ledger", "llmShare.panels.ledger"],
    ] as const) {
      const section = screen.getByTestId(testid);
      expect(section.textContent).toContain(t(key));
    }
  });

  it("全新实例空态齐备：offer 未发布 / allowlist 原话 / 账本两处空态", async () => {
    const { backend } = makeLlmShareMockPair();
    render(<LlmShareView backend={backend} />);
    expect(
      await screen.findByText(t("llmShare.offer.emptyTitle")),
    ).toBeTruthy();
    expect(
      await screen.findByText(t("llmShare.allowlist.emptyTitle")),
    ).toBeTruthy();
    expect(await screen.findByText(t("llmShare.ledger.emptyBalance"))).toBeTruthy();
    expect(await screen.findByText(t("llmShare.ledger.emptyList"))).toBeTruthy();
    expect(screen.getByLabelText(t("llmShare.borrow.formTargetPeer"))).toBeTruthy();
  });

  it("有数据时四面板同时渲染实体（互不阻塞）", async () => {
    const { backend, mock } = makeLlmShareMockPair({ now: () => 1788549300 });
    mock.offerPublish({ models: ["gpt-4o"], spare: { "gpt-4o": 7 }, periodEnds: "2026-09-30" });
    mock.allow({ peerId: "52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd" });
    mock.borrow({
      targetPeer: "52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd",
      model: "gpt-4o",
      messages: "hi",
      maxTokens: 8,
      reqId: "R1",
    });
    render(<LlmShareView backend={backend} />);
    expect(await screen.findByTestId("offer-status")).toBeTruthy();
    expect(await screen.findByTestId("allow-row")).toBeTruthy();
    expect(await screen.findByTestId("balance-row")).toBeTruthy();
    expect(await screen.findByTestId("ledger-row")).toBeTruthy();
  });
});
