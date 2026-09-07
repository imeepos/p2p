import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, useSearchParams } from "react-router-dom";
import { useEffect } from "react";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";

import "@/i18n";
import i18n from "@/i18n";

import { setBorrowPrefill, resetBorrowPrefill } from "./borrow-prefill";
import { BorrowPanel } from "./borrow-panel";
import { makeLlmShareMockPair } from "./mock-backend";
import { LlmShareView } from "./llm-share-view";

const t = i18n.t.bind(i18n);
const PEER = "52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd";

// 探测 query 是否已被视图清掉（防刷新重放断言）
function QueryProbe({ onParams }: { onParams: (params: URLSearchParams) => void }) {
  const [params] = useSearchParams();
  useEffect(() => {
    onParams(params);
  });
  return null;
}

afterEach(() => {
  cleanup();
  resetBorrowPrefill();
  vi.restoreAllMocks();
});

describe("borrow 预填（§6 第 5 步：redeem 后 query peer/model 预填并立即清 query）", () => {
  it("query 预填 targetPeer 与 model，并立即清 query 防刷新重放", async () => {
    const onParams = vi.fn();
    const { backend, mock } = makeLlmShareMockPair();
    mock.offerPublish({
      models: ["gpt-4o", "deepseek-v3"],
      spare: { "gpt-4o": 100, "deepseek-v3": 100 },
      periodEnds: "2026-09-30",
    });
    render(
      <MemoryRouter initialEntries={[`/llm-share?peer=${PEER}&model=gpt-4o`]}>
        <QueryProbe onParams={onParams} />
        <LlmShareView backend={backend} />
      </MemoryRouter>,
    );
    const peerInput = (await screen.findByLabelText(
      t("llmShare.borrow.formTargetPeer"),
    )) as HTMLInputElement;
    expect(peerInput.value).toBe(PEER);
    expect(
      (screen.getByLabelText(t("llmShare.borrow.formModel")) as HTMLInputElement).value,
    ).toBe("gpt-4o");
    await waitFor(() => {
      const calls = onParams.mock.calls as URLSearchParams[][];
      const last = calls[calls.length - 1]?.[0];
      expect(last.get("peer")).toBeNull();
      expect(last.get("model")).toBeNull();
    });
  });

  it("无 query 直接进入：表单为空（不预填），模型候选来自 offer 快照交接", async () => {
    setBorrowPrefill({ peer: PEER, model: "gpt-4o", models: ["gpt-4o", "deepseek-v3"] });
    const { backend } = makeLlmShareMockPair();
    render(
      <MemoryRouter initialEntries={["/llm-share"]}>
        <LlmShareView backend={backend} />
      </MemoryRouter>,
    );
    const peerInput = (await screen.findByLabelText(
      t("llmShare.borrow.formTargetPeer"),
    )) as HTMLInputElement;
    expect(peerInput.value).toBe(PEER);
    expect(
      (screen.getByLabelText(t("llmShare.borrow.formModel")) as HTMLInputElement).value,
    ).toBe("gpt-4o");
  });

  it("BorrowPanel 独立挂载消费 prefill 一次性：二次挂载不再重复预填", async () => {
    const { backend } = makeLlmShareMockPair();
    setBorrowPrefill({ peer: PEER, model: "m1", models: ["m1"] });
    const first = render(
      <MemoryRouter>
        <ConfirmProvider>
          <BorrowPanel backend={backend} />
        </ConfirmProvider>
      </MemoryRouter>,
    );
    const input = (await screen.findByLabelText(
      t("llmShare.borrow.formTargetPeer"),
    )) as HTMLInputElement;
    expect(input.value).toBe(PEER);
    first.unmount();
    cleanup();
    render(
      <MemoryRouter>
        <ConfirmProvider>
          <BorrowPanel backend={backend} />
        </ConfirmProvider>
      </MemoryRouter>,
    );
    const again = (await screen.findByLabelText(
      t("llmShare.borrow.formTargetPeer"),
    )) as HTMLInputElement;
    expect(again.value).toBe("");
  });
});