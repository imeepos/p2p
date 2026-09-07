import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import "@/i18n";
import i18n from "@/i18n";
import { formatDateTime } from "@/lib/format";
import { shortPeerId } from "@/lib/peer-name";
import { useChatStore } from "@/stores/chat-store";

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

function renderPanel(backend: LlmShareBackend) {
  return render(
    <ConfirmProvider>
      <AllowlistPanel backend={backend} />
    </ConfirmProvider>,
  );
}

function denyButton() {
  const row = screen.getByTestId("allow-row");
  const button = row.querySelector("button");
  if (!button) throw new Error("test precondition broken: deny button missing");
  return button;
}

afterEach(() => cleanup());

describe("LLM3 allowlist 面板（契约 §16.2-7 默认拒绝原话 / ai-guide 条目 1-3）", () => {
  it("空态渲染契约原话：默认拒绝说明", async () => {
    const { backend } = makeLlmShareMockPair();
    renderPanel(backend);
    expect(
      await screen.findByText(t("llmShare.allowlist.emptyTitle")),
    ).toBeTruthy();
  });

  it("allow 表单 models 缺省=不限模型，note 入列", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    renderPanel(backend);
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
  });

  // R2-02 回归：移出必须经破坏性二次确认，取消不动、确认才移除
  it("移出二次确认：取消保留条目，确认后移除并回空态", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    const denySpy = vi.spyOn(backend, "deny");
    renderPanel(backend);
    fireEvent.change(screen.getByLabelText(t("llmShare.allowlist.formPeerId")), {
      target: { value: PEER },
    });
    submitAllow();
    await screen.findByTestId("allow-row");
    fireEvent.click(denyButton());
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain(t("llmShare.allowlist.denyConfirmTitle"));
    expect(dialog.textContent).toContain(t("llmShare.allowlist.denyConfirmDesc"));
    expect(denySpy).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: t("common.actions.cancel") }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(screen.getByTestId("allow-row")).toBeTruthy();
    expect(denySpy).not.toHaveBeenCalled();
    fireEvent.click(denyButton());
    await screen.findByRole("alertdialog");
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.allowlist.deny") }));
    expect(await screen.findByText(t("llmShare.allowlist.emptyTitle"))).toBeTruthy();
    expect(mock.allowList().entries).toHaveLength(0);
  });

  // R2-04 回归：表头 i18n、已知好友昵称+缩略 PeerId、授权时间本地化
  it("表格表头走 i18n 借方列，行内已知好友显示昵称+缩略 PeerId", async () => {
    useChatStore.setState({
      friends: [{ peerId: PEER, nickname: "小借", addrs: [] }],
      friendsLoaded: true,
    });
    const { backend } = makeLlmShareMockPair();
    renderPanel(backend);
    fireEvent.change(screen.getByLabelText(t("llmShare.allowlist.formPeerId")), {
      target: { value: PEER },
    });
    submitAllow();
    const row = await screen.findByTestId("allow-row");
    expect(row.textContent).toContain("小借");
    expect(row.textContent).toContain(shortPeerId(PEER));
    expect(row.textContent).not.toContain(PEER);
    expect(
      screen.getByRole("columnheader", { name: t("llmShare.allowlist.columnPeer") }),
    ).toBeTruthy();
    expect(screen.queryByRole("columnheader", { name: "PeerId" })).toBeNull();
    useChatStore.setState({ friends: [], friendsLoaded: true });
  });

  it("授权时间本地化为 formatDateTime 而非原始 ISO UTC", async () => {
    const { backend, mock } = makeLlmShareMockPair({ now: () => 1788549300 });
    mock.allow({ peerId: PEER });
    renderPanel(backend);
    const row = await screen.findByTestId("allow-row");
    const grantedAt = mock.allowList().entries[0]!.grantedAt;
    const expected = formatDateTime(
      new Date(grantedAt).getTime(),
      i18n.language as "zh-CN",
    );
    expect(row.textContent).toContain(expected);
    expect(row.textContent).not.toContain("T0");
    expect(row.textContent).not.toContain("Z");
  });

  it("deny 失败路径显式报错（默认拒绝语义非故障，不静默）", async () => {
    const pair = makeLlmShareMockPair();
    pair.mock.allow({ peerId: PEER, models: ["gpt-4o"] });
    const deny = vi.fn(() => Promise.reject(new Error("deny: 条目不存在")));
    const backend = { ...pair.backend, deny } satisfies LlmShareBackend;
    renderPanel(backend);
    await screen.findByTestId("allow-row");
    fireEvent.click(denyButton());
    await screen.findByRole("alertdialog");
    fireEvent.click(screen.getByRole("button", { name: t("llmShare.allowlist.deny") }));
    expect(await screen.findByRole("alert")).toHaveTextContent("deny: 条目不存在");
  });
});
