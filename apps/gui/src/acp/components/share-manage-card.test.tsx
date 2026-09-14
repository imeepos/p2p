// 分享管理卡反馈接线测试（AF2）：刷新/重载 pending 与成功失败 toast、
// 撤销 pending 与成功失败 toast（确认弹框流程本身由 confirm-provider 测试覆盖）。
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { Toaster, toast } from "sonner";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { ShareManageCard } = await import("./share-manage-card");
const { ConfirmProvider } = await import("@/components/feedback/confirm-provider");
const { useAcpStore } = await import("../acp-store");
const { resetFixtures } = await import("../acp-view-test-utils");
const { resetToastDedupForTest } = await import("@/components/feedback/toast");
await import("@/i18n");

const ADMIN = "http://127.0.0.1:8123";

const OK = { ok: true, status: 200, json: async () => ({}) };

function shareRow(over: Record<string, unknown> = {}) {
  return {
    share_id: "s1",
    scope: "sandbox",
    workspace: null,
    allow_mcp: [] as string[],
    max_activations: 2,
    activations: 0,
    expires_at_unix: 0,
    revoked: false,
    note: "demo",
    created_at: "",
    bound_peer: null,
    ...over,
  };
}

function sharesResponse(rows: unknown[]) {
  return { ok: true, status: 200, json: async () => ({ shares: rows }) };
}

// 唯一的 admin 请求面：GET /shares 返回 rows，DELETE /shares/* 返回 init 决定
const fetchShares = (rows: unknown[], deleteImpl?: () => Promise<unknown>) =>
  vi.fn(async (url: string, init?: RequestInit) => {
    if (String(url).endsWith("/shares") && !init?.method) return sharesResponse(rows);
    if (init?.method === "DELETE") return deleteImpl ? deleteImpl() : OK;
    return OK;
  });

async function renderCard() {
  render(
    <ConfirmProvider>
      <ShareManageCard />
      <Toaster position="bottom-right" />
    </ConfirmProvider>,
  );
}

async function confirmDialog() {
  const dialog = await screen.findByRole("alertdialog");
  fireEvent.click(within(dialog).getByText("撤销"));
  // 冲刷确认 promise 的 .then 微任务，让行内按钮 pending 态先落定
  await act(async () => {});
}

beforeEach(() => {
  resetFixtures();
  resetToastDedupForTest();
  useAcpStore.setState({
    draft: { wsUrl: "ws://127.0.0.1:8787", token: "t", peer: "p", adminUrl: ADMIN, adminToken: "at" },
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  act(() => {
    toast.dismiss();
  });
});

describe("ShareManageCard 刷新反馈（AF2）", () => {
  it("点击刷新即 pending（disabled+aria-busy），完成后 toast「已刷新」", async () => {
    vi.stubGlobal("fetch", fetchShares([shareRow()]));
    await renderCard();
    expect(await screen.findByTestId("acp-share-row-s1")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-share-manage-refresh"));
    const btn = screen.getByTestId("acp-share-manage-refresh");
    expect(btn.hasAttribute("disabled")).toBe(true);
    expect(btn.getAttribute("aria-busy")).toBe("true");
    await screen.findByText("已刷新");
  });

  it("刷新失败：toast「刷新失败」且行内错误面板出现（持久留痕不依赖 toast）", async () => {
    let fail = false;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string, init?: RequestInit) => {
        if (String(url).endsWith("/shares") && !init?.method) {
          return fail
            ? { ok: false, status: 500, json: async () => ({ error: "boom" }) }
            : sharesResponse([shareRow()]);
        }
        return OK;
      }),
    );
    await renderCard();
    expect(await screen.findByTestId("acp-share-row-s1")).toBeTruthy();
    fail = true;
    fireEvent.click(screen.getByTestId("acp-share-manage-refresh"));
    await screen.findByText("刷新失败");
    expect(screen.getByTestId("acp-share-manage-error")).toBeTruthy();
  });

  it("错误面板「重新加载」：点击 pending 后失败 toast 再次出现", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => ({ ok: false, status: 500, json: async () => ({ error: "boom" }) })),
    );
    await renderCard();
    expect(await screen.findByTestId("acp-share-manage-error")).toBeTruthy();
    fireEvent.click(screen.getByTestId("acp-share-manage-reload"));
    expect(screen.getByTestId("acp-share-manage-reload").hasAttribute("disabled")).toBe(true);
    await screen.findByText("刷新失败");
  });
});

describe("ShareManageCard 撤销反馈（AF2）", () => {
  it("撤销确认后 pending 期间按钮禁用，成功 toast「分享已撤销」", async () => {
    let resolveDelete!: () => void;
    vi.stubGlobal(
      "fetch",
      fetchShares([shareRow()], () => new Promise<unknown>((r) => {
        resolveDelete = () => r(OK);
      })),
    );
    await renderCard();
    fireEvent.click(await screen.findByTestId("acp-share-revoke-s1"));
    await confirmDialog();
    const btn = screen.getByTestId("acp-share-revoke-s1");
    expect(btn.hasAttribute("disabled")).toBe(true);
    resolveDelete();
    await screen.findByText("分享已撤销");
  });

  it("撤销失败：toast「撤销失败」且按钮恢复可点", async () => {
    vi.stubGlobal(
      "fetch",
      fetchShares([shareRow()], async () => ({
        ok: false,
        status: 500,
        json: async () => ({ error: "boom" }),
      })),
    );
    await renderCard();
    fireEvent.click(await screen.findByTestId("acp-share-revoke-s1"));
    await confirmDialog();
    await screen.findByText("撤销失败");
    expect(screen.getByTestId("acp-share-revoke-s1").hasAttribute("disabled")).toBe(false);
  });
});
