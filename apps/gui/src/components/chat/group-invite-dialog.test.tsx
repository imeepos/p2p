import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ChatMessageJson, GroupInviteJson } from "@/lib/ipc-types";

import "@/i18n";

// 入群确认弹框（IMC3 需求 1）：同意/拒绝双按钮、失败原文上浮不静默、
// 邀请簿未同步的防御路径、成功后经 onAccepted 上抛 groupId 由调用方跳转。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    chatGroupInviteAccept: vi.fn(),
    chatGroupInviteReject: vi.fn(),
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatGroupInviteAccept: mocks.chatGroupInviteAccept,
    chatGroupInviteReject: mocks.chatGroupInviteReject,
  },
}));

import { GroupInviteDialog, type GroupInviteTarget } from "./group-invite-dialog";

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
(window as unknown as { ResizeObserver: unknown }).ResizeObserver = ResizeObserverStub;
if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {};
}

const cardMessage: ChatMessageJson = {
  id: "m1",
  peer: "peer-b",
  sender: "them",
  kind: "groupInvite",
  tsMs: 1_700_000_000_000,
  text: null,
  media: null,
  status: "delivered",
  replyTo: null,
  groupInvite: {
    groupId: "g-1",
    groupName: "项目组",
    inviterNickname: "阿北",
    note: "周末副本",
  },
};

const invite: GroupInviteJson = {
  id: "inv-1",
  groupId: "g-1",
  groupName: "项目组",
  owner: "owner-peer-xyz",
  inviter: "peer-b",
  invitee: "self",
  note: null,
  direction: "in",
  state: "pending",
  tsMs: 1000,
  delivered: true,
};

function LocationProbe() {
  const location = useLocation();
  return <div data-testid="loc">{location.pathname + location.search}</div>;
}

function renderDialog(target: GroupInviteTarget | null) {
  const onOpenChange = vi.fn();
  render(
    <MemoryRouter initialEntries={["/start"]}>
      <LocationProbe />
      <GroupInviteDialog target={target} onOpenChange={onOpenChange} />
    </MemoryRouter>,
  );
  return { onOpenChange };
}

function probeUrl(): string {
  return screen.getByTestId("loc").textContent ?? "";
}

beforeEach(() => {
  mocks.chatGroupInviteAccept.mockReset();
  mocks.chatGroupInviteReject.mockReset();
});

describe("GroupInviteDialog", () => {
  it("群信息/邀请人/备注/成员数口径齐备", () => {
    renderDialog({ message: cardMessage, invite });
    expect(screen.getByTestId("group-invite-dialog-group").textContent).toBe("项目组");
    expect(screen.getByTestId("group-invite-dialog-owner").textContent).toContain("owner-peer");
    expect(screen.getByTestId("group-invite-dialog-inviter").textContent).toBe("阿北");
    expect(screen.getByTestId("group-invite-dialog-note").textContent).toBe("周末副本");
    expect(screen.getByText("成员数以同意进群后的群名单为准")).toBeTruthy();
    cleanup();
  });

  it("同意成功：IPC 带邀请 id 调用，跳转 /chat?group= 且面板关闭", async () => {
    mocks.chatGroupInviteAccept.mockResolvedValue(undefined);
    const { onOpenChange } = renderDialog({ message: cardMessage, invite });
    fireEvent.click(screen.getByTestId("group-invite-accept"));
    await waitFor(() => expect(probeUrl()).toBe("/chat?group=g-1"));
    expect(mocks.chatGroupInviteAccept).toHaveBeenCalledWith("inv-1");
    expect(onOpenChange).toHaveBeenCalledWith(false);
    cleanup();
  });

  it("同意失败：原文上浮 role=alert，面板不关闭不跳转", async () => {
    mocks.chatGroupInviteAccept.mockRejectedValue(new Error("名单版本冲突"));
    const { onOpenChange } = renderDialog({ message: cardMessage, invite });
    fireEvent.click(screen.getByTestId("group-invite-accept"));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("名单版本冲突");
    expect(probeUrl()).toBe("/start");
    expect(onOpenChange).not.toHaveBeenCalled();
    cleanup();
  });

  it("拒绝成功：调用 reject 且面板关闭，不触发跳转", async () => {
    mocks.chatGroupInviteReject.mockResolvedValue(undefined);
    const { onOpenChange } = renderDialog({ message: cardMessage, invite });
    fireEvent.click(screen.getByTestId("group-invite-reject"));
    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
    expect(mocks.chatGroupInviteReject).toHaveBeenCalledWith("inv-1", null);
    expect(probeUrl()).toBe("/start");
    cleanup();
  });

  it("拒绝失败：原文上浮", async () => {
    mocks.chatGroupInviteReject.mockRejectedValue(new Error("网络不可达"));
    renderDialog({ message: cardMessage, invite });
    fireEvent.click(screen.getByTestId("group-invite-reject"));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("网络不可达");
    cleanup();
  });

  it("邀请簿未同步（invite=null）：同意给防御提示不发 IPC", async () => {
    renderDialog({ message: cardMessage, invite: null });
    fireEvent.click(screen.getByTestId("group-invite-accept"));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("邀请状态未同步");
    expect(mocks.chatGroupInviteAccept).not.toHaveBeenCalled();
    cleanup();
  });
});
