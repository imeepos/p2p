// 创建分享弹层发送按钮反馈测试（AF2）：发送到聊天按钮补 pending（AsyncButton），
// 失败 toast「发送失败」为既有链路（sendToChat catch），此处作回归锚。
import { act, fireEvent, render, screen } from "@testing-library/react";
import { Toaster, toast } from "sonner";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { ShareCreateDialog } = await import("./share-create-dialog");
const { useAcpStore } = await import("../acp-store");
const { resetFixtures } = await import("../acp-view-test-utils");
const { resetToastDedupForTest } = await import("@/components/feedback/toast");
await import("@/i18n");

const ADMIN = "http://127.0.0.1:8123";
const LINK = "dsh-acp-share://p2p-test";

const OK = { ok: true, status: 200, json: async () => ({}) };

function stubCreateFetch() {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      if (String(url).endsWith("/shares") && init?.method === "POST") {
        return {
          ok: true,
          status: 200,
          json: async () => ({ share_id: "s1", token: "stok", link: LINK }),
        };
      }
      return OK;
    }),
  );
}

async function renderDialog(onSendLink: (link: string) => Promise<void>, onOpenChange: (open: boolean) => void) {
  render(
    <>
      <ShareCreateDialog open onOpenChange={onOpenChange} onSendLink={onSendLink} />
      <Toaster position="bottom-right" />
    </>,
  );
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

describe("ShareCreateDialog 发送反馈（AF2）", () => {
  it("发送到聊天：pending 期间按钮禁用，成功后回调关闭弹框", async () => {
    stubCreateFetch();
    const onOpenChange = vi.fn();
    let resolveSend!: () => void;
    const onSendLink = vi.fn(
      () => new Promise<void>((r) => {
        resolveSend = r;
      }),
    );
    await renderDialog(onSendLink, onOpenChange);
    fireEvent.click(await screen.findByTestId("acp-share-create"));
    const sendBtn = await screen.findByTestId("acp-share-send");
    fireEvent.click(sendBtn);
    expect(onSendLink).toHaveBeenCalledWith(LINK);
    const busyBtn = screen.getByTestId("acp-share-send");
    expect(busyBtn.hasAttribute("disabled")).toBe(true);
    expect(busyBtn.getAttribute("aria-busy")).toBe("true");
    resolveSend();
    await vi.waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
  });

  it("发送失败：toast「发送失败」出现且弹框保持打开", async () => {
    stubCreateFetch();
    const onOpenChange = vi.fn();
    const onSendLink = vi.fn(async () => {
      throw new Error("peer offline");
    });
    await renderDialog(onSendLink, onOpenChange);
    fireEvent.click(await screen.findByTestId("acp-share-create"));
    fireEvent.click(await screen.findByTestId("acp-share-send"));
    await screen.findByText("发送失败");
    expect(onOpenChange).not.toHaveBeenCalledWith(false);
  });
});

// AF2 修复轮 1：listWorkspaces 去吞错后，弹层内 workspace 拉取失败必须有
// 可见反馈（行内错误提示），同时保留「失败回落默认工作区」既有语义。
describe("ShareCreateDialog 工作区清单失败反馈（AF2）", () => {
  it("workspace 拉取失败：行内错误提示出现且回落提示仍在", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string, _init?: RequestInit) => {
        if (String(url).endsWith("/workspaces")) {
          return { ok: false, status: 500, json: async () => ({ error: "boom" }) };
        }
        return OK;
      }),
    );
    const onOpenChange = vi.fn();
    const onSendLink = vi.fn(async () => {});
    await renderDialog(onSendLink, onOpenChange);
    fireEvent.click(await screen.findByTestId("acp-share-scope"));
    fireEvent.click(await screen.findByRole("option", { name: "工作区目录" }));
    expect(await screen.findByTestId("acp-share-ws-error")).toBeTruthy();
    expect(screen.getByText("工作区范围要求 agent 已配置工作区目录，否则创建即被拒绝")).toBeTruthy();
  });
});
