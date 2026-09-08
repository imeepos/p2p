import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { Toaster, toast } from "sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FriendInviteJson } from "@/lib/ipc-types";

import "@/i18n";

// 好友邀请行级交互（F08 撤回 / F21 缩略 PeerId 复制）：与页级拆分防超
// 行数红线。IPC mock 命令名与契约逐字一致。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    chatInvitesList: vi.fn(),
    chatGroupInvitesList: vi.fn(),
    chatGroupInviteAccept: vi.fn(),
    chatGroupInviteReject: vi.fn(),
    chatInviteAccept: vi.fn(),
    chatInviteReject: vi.fn(),
    chatInviteCancel: vi.fn(),
    chatFriendsList: vi.fn(),
    chatHistory: vi.fn(),
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatInvitesList: mocks.chatInvitesList,
    chatGroupInvitesList: mocks.chatGroupInvitesList,
    chatGroupInviteAccept: mocks.chatGroupInviteAccept,
    chatGroupInviteReject: mocks.chatGroupInviteReject,
    chatInviteAccept: mocks.chatInviteAccept,
    chatInviteReject: mocks.chatInviteReject,
    chatInviteCancel: mocks.chatInviteCancel,
    chatFriendsList: mocks.chatFriendsList,
    chatHistory: mocks.chatHistory,
  },
}));

import { useChatStore } from "@/stores/chat-store";
import { MessagesPage } from "./messages-page";

const PEER_OUT = "peer-out-bbbbbbbb";
// F21 用真实形态 44 位 PeerId 验证缩略(title 悬停全文)与复制全文
const PEER_OUT_LONG = "KGNC6K7tqwmftgTQWLETF2y5uo2pbYeUNbPA7D3wAxjg";

const friendOut: FriendInviteJson = {
  peerId: PEER_OUT,
  nickname: "小圆",
  addrs: [],
  note: null,
  direction: "out",
  tsMs: 1_700_000_003_000,
  delivered: false,
};

function renderPage() {
  render(
    <MemoryRouter initialEntries={["/messages"]}>
      <MessagesPage />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  for (const fn of Object.values(mocks)) {
    if (typeof fn === "function") (fn as ReturnType<typeof vi.fn>).mockReset();
  }
  mocks.chatInvitesList.mockResolvedValue([friendOut]);
  mocks.chatGroupInvitesList.mockResolvedValue([]);
  mocks.chatFriendsList.mockResolvedValue([]);
  mocks.chatHistory.mockResolvedValue([]);
  useChatStore.setState({
    groupInvites: [],
    groupInvitesLoaded: false,
    groupInvitesError: null,
    invites: [],
    friends: [],
    friendsLoaded: true,
    friendsError: null,
  });
});

describe("好友邀请行级交互", () => {
  it("F08：发出的邀请卡有「撤回」，与通讯录同标签同 store action", async () => {
    mocks.chatInviteCancel.mockResolvedValue(undefined);
    renderPage();
    fireEvent.click(await screen.findByTestId("messages-friend-withdraw-" + PEER_OUT));
    await waitFor(() => expect(mocks.chatInviteCancel).toHaveBeenCalledWith(PEER_OUT));
    expect(mocks.chatInviteAccept).not.toHaveBeenCalled();
  });

  it("F08：撤回失败原文上浮 role=alert", async () => {
    mocks.chatInviteCancel.mockRejectedValue(new Error("邀请已被处理"));
    const logSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    renderPage();
    fireEvent.click(await screen.findByTestId("messages-friend-withdraw-" + PEER_OUT));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("撤回失败：");
    expect(alert.textContent).toContain("邀请已被处理");
    logSpy.mockRestore();
  });

  // 终验 F21 复盘：缩略 PeerId 复制按钮实际已交付（90ce353），走查端在
  // 纯图标 ghost 按钮上漏检。此用例按终验同款真实交互路径断言按钮存在、
  // 复制全文、toast 反馈，防「假阴性/假绿」两头复发。
  it("F21：发出的邀请卡缩略 PeerId 带复制按钮，点击复制全文并 toast 反馈", async () => {
    mocks.chatInvitesList.mockResolvedValue([
      { ...friendOut, peerId: PEER_OUT_LONG },
    ]);
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    render(
      <MemoryRouter initialEntries={["/messages"]}>
        <MessagesPage />
        <Toaster position="bottom-right" />
      </MemoryRouter>,
    );
    const row = await screen.findByTestId("messages-friend-row-" + PEER_OUT_LONG);
    // 缩略显示 + 悬停全文（span title 44 位完整 PeerId）
    const brief = row.querySelector("span[title]");
    expect(brief).not.toBeNull();
    expect(brief!.getAttribute("title")).toBe(PEER_OUT_LONG);
    expect(brief!.textContent).not.toBe(PEER_OUT_LONG);
    // 复制按钮真实可点，写入完整 PeerId
    fireEvent.click(within(row).getByRole("button", { name: "复制" }));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith(PEER_OUT_LONG));
    // 复制成功 toast 反馈
    await screen.findByText("已复制到剪贴板");
    act(() => {
      toast.dismiss();
    });
  });
});
