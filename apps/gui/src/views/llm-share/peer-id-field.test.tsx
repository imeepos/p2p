import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import "@/i18n";
import i18n from "@/i18n";
import { useNodeStore } from "@/stores/node-store";

import { makeLlmShareMockPair } from "./mock-backend";
import { AllowlistPanel } from "./allowlist-panel";
import { BorrowPanel } from "./borrow-panel";

const t = i18n.t.bind(i18n);
const PEER = "52REhUoptPD8V99TtwHzBoczLTDXGTy8dk9aaxVbiJwd";
const PEER_B = "7V8SRkBS6XLhS731XBcYbpjGBDctApRsbo49w2xhJGSk";
const FAKE = "alice-fake-peer";

function renderWithConfirm(ui: React.ReactElement) {
  return render(<ConfirmProvider>{ui}</ConfirmProvider>);
}

function seedNodePeers(peerIds: string[]) {
  useNodeStore.setState({
    peers: Object.fromEntries(
      peerIds.map((peerId) => [
        peerId,
        { peerId, addrs: [], source: "manual" as const, connected: true, lastSeenMs: 1, hops: [] },
      ]),
    ),
  });
}

function fillBorrowPeer(value: string) {
  fireEvent.change(screen.getByLabelText(t("llmShare.borrow.formTargetPeer")), {
    target: { value },
  });
}

function submitForm(buttonName: string) {
  const form = screen.getByRole("button", { name: buttonName }).closest("form");
  if (!form) throw new Error("test precondition broken: form missing");
  fireEvent.submit(form);
}

afterEach(() => {
  cleanup();
  useNodeStore.setState({ peers: {} });
});

describe("R2-05 PeerId 关联输入：节点选择器 + base58/32 字节即时校验", () => {
  it("白名单：选择器候选来自节点表，选中即回填输入框", async () => {
    seedNodePeers([PEER, PEER_B]);
    const { backend } = makeLlmShareMockPair();
    renderWithConfirm(<AllowlistPanel backend={backend} />);
    fireEvent.click(screen.getByTestId("llm-allow-peer-pick"));
    fireEvent.click(await screen.findByTestId("llm-allow-peer-pick-panel"));
    const option = screen.getByRole("option", { name: new RegExp(PEER_B.slice(0, 6)) });
    fireEvent.click(option);
    const input = screen.getByLabelText(t("llmShare.allowlist.formPeerId")) as HTMLInputElement;
    expect(input.value).toBe(PEER_B);
  });

  it("白名单：失焦即时报格式错误；非法值提交被拦截不触达后端", async () => {
    const { backend } = makeLlmShareMockPair();
    const allowSpy = vi.spyOn(backend, "allow");
    renderWithConfirm(<AllowlistPanel backend={backend} />);
    const input = screen.getByLabelText(t("llmShare.allowlist.formPeerId"));
    fireEvent.change(input, { target: { value: FAKE } });
    expect(screen.queryByRole("alert")).toBeNull();
    fireEvent.blur(input);
    expect(screen.getByRole("alert").textContent).toContain(
      t("llmShare.allowlist.errPeerInvalid"),
    );
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(input.getAttribute("aria-describedby")).toBe("llm-allow-peer-error");
    submitForm(t("llmShare.allowlist.allow"));
    expect(allowSpy).not.toHaveBeenCalled();
  });

  it("白名单：合法 PeerId 提交通过（自由文本兜底不破坏原路径）", async () => {
    const { backend } = makeLlmShareMockPair();
    const allowSpy = vi.spyOn(backend, "allow");
    renderWithConfirm(<AllowlistPanel backend={backend} />);
    fireEvent.change(screen.getByLabelText(t("llmShare.allowlist.formPeerId")), {
      target: { value: PEER },
    });
    submitForm(t("llmShare.allowlist.allow"));
    await waitFor(() => expect(allowSpy).toHaveBeenCalledTimes(1));
  });

  it("借用：出借方 PeerId 格式非法时提交被拦截给字段错误", async () => {
    const { backend } = makeLlmShareMockPair();
    const borrowSpy = vi.spyOn(backend, "borrow");
    renderWithConfirm(<BorrowPanel backend={backend} />);
    fillBorrowPeer("zzzNotAllowlisted99");
    fireEvent.change(screen.getByLabelText(t("llmShare.borrow.formModel")), {
      target: { value: "gpt-4o" },
    });
    fireEvent.change(screen.getByLabelText(t("llmShare.borrow.formMaxTokens")), {
      target: { value: "128" },
    });
    fireEvent.change(screen.getByLabelText(t("llmShare.borrow.formMessages")), {
      target: { value: "hi" },
    });
    submitForm(t("llmShare.borrow.submit"));
    const alerts = await screen.findAllByRole("alert");
    expect(alerts.map((a) => a.textContent).join(" ")).toContain(
      t("llmShare.borrow.errTargetPeerFormat"),
    );
    expect(borrowSpy).not.toHaveBeenCalled();
  });
});
