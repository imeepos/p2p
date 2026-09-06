import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ChatMessageJson, GroupInviteJson } from "@/lib/ipc-types";

import { GroupInviteCard } from "./group-invite-card";
import "@/i18n";

// 入群邀请卡片各状态渲染（IMC3 需求 1）：me 卡方向+状态徽章、them 卡
// 待处理可点开弹框、已处理点击给提示态不进弹框。

function cardMessage(patch: Partial<ChatMessageJson> = {}): ChatMessageJson {
  return {
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
    ...patch,
  };
}

function inviteOf(patch: Partial<GroupInviteJson>): GroupInviteJson {
  return {
    id: "inv-1",
    groupId: "g-1",
    groupName: "项目组",
    owner: "owner",
    inviter: "peer-b",
    invitee: "self",
    note: null,
    direction: "in",
    state: "pending",
    tsMs: 1000,
    delivered: true,
    ...patch,
  };
}

function renderCard(
  message: ChatMessageJson,
  invite: GroupInviteJson | null = null,
) {
  const onOpenConfirm = vi.fn();
  render(
    <GroupInviteCard message={message} invite={invite} onOpenConfirm={onOpenConfirm} />,
  );
  return onOpenConfirm;
}

describe("GroupInviteCard 渲染矩阵", () => {
  it("them 待处理卡：群名/邀请人/备注/状态齐备且整卡可点", () => {
    const onOpenConfirm = renderCard(cardMessage(), inviteOf({}));
    const card = screen.getByTestId("group-invite-card");
    expect(card.textContent).toContain("项目组");
    expect(card.textContent).toContain("邀请你加入群聊");
    expect(card.textContent).toContain("邀请人：阿北");
    expect(card.textContent).toContain("备注：周末副本");
    expect(screen.getByTestId("group-invite-state").textContent).toBe("待处理");
    expect(card.tagName).toBe("BUTTON");
    fireEvent.click(card);
    expect(onOpenConfirm).toHaveBeenCalledTimes(1);
  });

  it("them 已同意卡：点击不进弹框（提示态路径）", () => {
    const onOpenConfirm = renderCard(cardMessage(), inviteOf({ state: "accepted" }));
    const card = screen.getByTestId("group-invite-card");
    expect(screen.getByTestId("group-invite-state").textContent).toBe("已同意");
    expect(card.tagName).toBe("DIV");
    fireEvent.click(card);
    expect(onOpenConfirm).not.toHaveBeenCalled();
  });

  it("them 已拒绝卡：点击不进弹框", () => {
    const onOpenConfirm = renderCard(cardMessage(), inviteOf({ state: "rejected" }));
    expect(screen.getByTestId("group-invite-state").textContent).toBe("已拒绝");
    fireEvent.click(screen.getByTestId("group-invite-card"));
    expect(onOpenConfirm).not.toHaveBeenCalled();
  });

  it("me 卡：方向徽标「发出的」+ 状态徽章，不可点进弹框", () => {
    const onOpenConfirm = renderCard(cardMessage({ sender: "me" }), inviteOf({}));
    const card = screen.getByTestId("group-invite-card");
    expect(card.textContent).toContain("已发起入群邀请");
    expect(card.textContent).toContain("发出的");
    expect(screen.getByTestId("group-invite-state").textContent).toBe("待处理");
    expect(card.tagName).toBe("DIV");
    fireEvent.click(card);
    expect(onOpenConfirm).not.toHaveBeenCalled();
  });

  it("me 卡状态推进渲染：已同意/已拒绝", () => {
    renderCard(cardMessage({ sender: "me" }), inviteOf({ state: "accepted" }));
    expect(screen.getByTestId("group-invite-state").textContent).toBe("已同意");
    cleanup();
    renderCard(cardMessage({ sender: "me" }), inviteOf({ state: "rejected" }));
    expect(screen.getByTestId("group-invite-state").textContent).toBe("已拒绝");
  });

  it("邀请簿未同步（invite=null）按待处理呈现且可点", () => {
    const onOpenConfirm = renderCard(cardMessage());
    expect(screen.getByTestId("group-invite-state").textContent).toBe("待处理");
    fireEvent.click(screen.getByTestId("group-invite-card"));
    expect(onOpenConfirm).toHaveBeenCalledWith(null);
  });
});
