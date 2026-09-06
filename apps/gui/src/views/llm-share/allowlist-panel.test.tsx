import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import "@/i18n";
import i18n from "@/i18n";

import { makeLlmShareMockPair } from "./mock-backend";
import { AllowlistPanel } from "./allowlist-panel";
import type { LlmShareBackend } from "./types";

const t = i18n.t.bind(i18n);
const PEER = "52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd";

function submitAllow() {
  const button = screen.getByRole("button", { name: t("llmShare.allowlist.allow") });
  const form = button.closest("form");
  if (!form) throw new Error("test precondition broken: allow form missing");
  fireEvent.submit(form);
}

afterEach(() => cleanup());

describe("LLM3 allowlist 面板（契约 §16.2-7 默认拒绝原话 / ai-guide 条目 1-3）", () => {
  it("空态渲染契约原话：allowlist 无条目即不可用", async () => {
    const { backend } = makeLlmShareMockPair();
    render(<AllowlistPanel backend={backend} />);
    expect(
      await screen.findByText(t("llmShare.allowlist.emptyTitle")),
    ).toBeTruthy();
  });

  it("allow 表单 models 缺省=不限模型，note 入列并可移出（deny 回空态）", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    render(<AllowlistPanel backend={backend} />);
    fireEvent.change(screen.getByLabelText(t("llmShare.allowlist.formPeerId")), {
      target: { value: PEER },
    });
    fireEvent.change(screen.getByLabelText(t("llmShare.allowlist.formNote")), {
      target: { value: "首批白名单" },
    });
    submitAllow();
    const row = await screen.findByTestId("allow-row");
    expect(row.textContent).toContain(t("llmShare.allowlist.unlimitedModels"));
    expect(row.textContent).toContain("首批白名单");
    expect(mock.allowList().entries[0].models).toEqual([]);
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.allowlist.deny") }));
    expect(await screen.findByText(t("llmShare.allowlist.emptyTitle"))).toBeTruthy();
  });

  it("deny 失败路径显式报错（默认拒绝语义非故障，不静默）", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    mock.allow({ peerId: PEER, models: ["gpt-4o"] });
    const spy = {
      ...backend,
      deny: (peerId: string) => {
        void peerId;
        return Promise.reject(new Error("deny: 条目不存在"));
      },
    } satisfies LlmShareBackend;
    render(<AllowlistPanel backend={spy} />);
    const row = await screen.findByTestId("allow-row");
    fireEvent.click(
      row.querySelector('button, [role="button"]') ??
        (() => {
          throw new Error("test precondition broken: deny button missing");
        })(),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("deny: 条目不存在");
  });
});
