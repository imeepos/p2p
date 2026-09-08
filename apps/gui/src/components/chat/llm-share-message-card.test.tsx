import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, useLocation } from "react-router-dom";

import "@/i18n";
import i18n from "@/i18n";

import { makeLlmShareMockPair } from "@/views/llm-share/mock-backend";
import { resetBorrowPrefill } from "@/views/llm-share/borrow-prefill";
import { LLM_SHARE_LINK_PREFIX } from "@/lib/llm-share-link-model";

import { LlmShareMessageCard, TextWithLlmShareLink } from "./llm-share-message-card";

const t = i18n.t.bind(i18n);

const { toastSuccessMock, toastErrorMock } = vi.hoisted(() => ({
  toastSuccessMock: vi.fn(),
  toastErrorMock: vi.fn(),
}));
vi.mock("@/components/feedback/toast", () => ({
  toastSuccess: toastSuccessMock,
  toastError: toastErrorMock,
}));

const TOKEN = "0123456789abcdef0123456789abcdef";
const LENDER = "7V8SRkBS6XLhS731XBcYbpjGBDctApRsbo49w2xhJGSk";

function linkOf(overrides: Record<string, string> = {}): string {
  const params = new URLSearchParams({
    peer: LENDER,
    token: TOKEN,
    exp: "1788549300",
    sid: "sh-1",
    models: "gpt-4o",
    ...overrides,
  });
  return LLM_SHARE_LINK_PREFIX + params.toString();
}

function LocationProbe({ onPath }: { onPath: (path: string) => void }) {
  const location = useLocation();
  onPath(location.pathname + location.search);
  return null;
}

function renderCard(
  link: string,
  backend: Parameters<typeof LlmShareMessageCard>[0]["backend"],
  opts: { onPath?: (path: string) => void } = {},
) {
  return render(
    <MemoryRouter initialEntries={["/chat"]}>
      {opts.onPath ? <LocationProbe onPath={opts.onPath} /> : null}
      <LlmShareMessageCard link={link} backend={backend} />
    </MemoryRouter>,
  );
}

afterEach(() => {
  cleanup();
  resetBorrowPrefill();
  vi.clearAllMocks();
});

describe("llm-share 聊天卡片（scheme dsh-llm-share://，独立域）", () => {
  it("渲染卡片：标题/提示/加入按钮", () => {
    renderCard(linkOf(), makeLlmShareMockPair().backend);
    expect(screen.getByTestId("chat-llm-share-card")).toBeTruthy();
    expect(screen.getByText(t("chat.llmShareMessage.title"))).toBeTruthy();
    expect(screen.getByTestId("chat-llm-share-join")).toBeTruthy();
  });

  it("点击兑换成功：toast + 跳 /llm-share?peer=&model= + borrow 预填", async () => {
    const onPath = vi.fn();
    const { backend, mock } = makeLlmShareMockPair();
    mock.providerSave({
      id: "pv-1",
      name: "P",
      baseUrl: "https://api.example.com/v1",
      protocol: "openai",
      apiKey: "sk-test-1234567890abcd",
      models: ["gpt-4o"],
    });
    mock.offerPublish({
      models: ["gpt-4o"],
      spare: { "gpt-4o": 100 },
      periodEnds: "2026-09-30",
    });
    const created = mock.shareCreate({ providerId: "pv-1" });
    renderCard(created.link, backend, { onPath });
    fireEvent.click(screen.getByTestId("chat-llm-share-join"));
    await waitFor(() => expect(toastSuccessMock).toHaveBeenCalledWith(t("chat.llmShareMessage.redeemed")));
    expect(screen.getByText(t("chat.llmShareMessage.joined"))).toBeTruthy();
    const calls = onPath.mock.calls as string[][];
    const path = calls[calls.length - 1]?.[0] as string;
    expect(path).toContain("/llm-share?peer=" + LENDER);
    expect(path).toContain("model=gpt-4o");
  });

  it("业务拒绝码原样透出：revoked 显示对应文案不本地化改写", async () => {
    const { backend, mock } = makeLlmShareMockPair();
    mock.seedShare({
      token: TOKEN,
      models: ["gpt-4o"],
      revoked: true,
    });
    renderCard(linkOf(), backend);
    fireEvent.click(screen.getByTestId("chat-llm-share-join"));
    expect(await screen.findByTestId("chat-llm-share-denied")).toBeTruthy();
    expect(screen.getByTestId("chat-llm-share-denied").textContent).toContain(
      t("chat.llmShareMessage.codeRevoked"),
    );
  });

  it("TextWithLlmShareLink：正文识别链接成卡片，其余文字保留", () => {
    const link = linkOf();
    render(
      <MemoryRouter>
        <TextWithLlmShareLink text={"前缀 " + link + " 后缀"} />
      </MemoryRouter>,
    );
    expect(screen.getByText("前缀")).toBeTruthy();
    expect(screen.getByText("后缀")).toBeTruthy();
    expect(screen.getByTestId("chat-llm-share-card")).toBeTruthy();
  });

  it("无链接走纯文本，不误渲染卡片", () => {
    render(
      <MemoryRouter>
        <TextWithLlmShareLink text="普通文本" />
      </MemoryRouter>,
    );
    expect(screen.queryByTestId("chat-llm-share-card")).toBeNull();
    expect(screen.getByText("普通文本")).toBeTruthy();
  });
});