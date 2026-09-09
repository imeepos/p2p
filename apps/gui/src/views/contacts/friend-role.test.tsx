import { useState } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AuthzRoleView, ChatFriendJson } from "@/lib/ipc-types";
import { useAuthzStore } from "@/stores/authz-store";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    authzBind: vi.fn<(peerId: string, roleId: string, expiresAt: number | null, note: string | null) => Promise<unknown>>(),
    authzUnbind: vi.fn<(peerId: string) => Promise<unknown>>(),
    authzRoleList: vi.fn<() => Promise<{ roles: AuthzRoleView[] }>>(),
    authzBindingsList: vi.fn<() => Promise<{ bindings: unknown[] }>>(),
    authzDefaultRoleGet: vi.fn<() => Promise<{ roleId: string }>>(),
    authzDefaultRoleSave: vi.fn<(roleId: string) => Promise<{ roleId: string }>>(),
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    authzBind: mocks.authzBind,
    authzUnbind: mocks.authzUnbind,
    authzRoleList: mocks.authzRoleList,
    authzBindingsList: mocks.authzBindingsList,
    authzDefaultRoleGet: mocks.authzDefaultRoleGet,
    authzDefaultRoleSave: mocks.authzDefaultRoleSave,
  },
}));

import "@/i18n";
import { FriendRoleBadge, RoleUnboundHint } from "@/views/contacts/friend-role-badge";
import { FriendRoleDialog } from "@/views/contacts/friend-role-dialog";

// radix select 滚动定位依赖 scrollIntoView，jsdom 未实现（palette-nav 测试桩先例）
if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {};
}

// 真实 base58（解码恰 32 字节），与后端 peer 校验同口径的合法夹具。
const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";

const ROLES: AuthzRoleView[] = [
  { roleId: "friend", name: "好友", permissions: ["chat.send"], builtin: true, note: "" },
  { roleId: "ally", name: "盟友", permissions: ["chat.send", "llm.borrow"], builtin: true, note: "" },
];

function friendOf(peerId: string): ChatFriendJson {
  return { peerId, nickname: "小圆", addrs: [], note: null };
}

function seedStore(bindings: Record<string, { peerId: string; roleId: string; grantedAt: number; note: string; expiresAt?: number }>): void {
  useAuthzStore.setState({
    roles: ROLES,
    bindings,
    defaultRoleId: "friend",
    loadError: null,
  });
}

beforeEach(() => {
  mocks.authzBind.mockReset().mockResolvedValue({});
  mocks.authzUnbind.mockReset().mockResolvedValue({});
  mocks.authzRoleList.mockReset().mockResolvedValue({ roles: ROLES });
  mocks.authzBindingsList.mockReset().mockResolvedValue({ bindings: [] });
  mocks.authzDefaultRoleGet.mockReset().mockResolvedValue({ roleId: "friend" });
  mocks.authzDefaultRoleSave.mockReset().mockResolvedValue({ roleId: "" });
  seedStore({});
});

describe("FriendRoleBadge 角色徽章", () => {
  it("有绑定时展示角色名徽章，未绑定不渲染", () => {
    const { rerender } = render(<FriendRoleBadge peerId={PEER} />);
    expect(screen.queryByTestId("contact-friend-role-" + PEER)).toBeNull();

    seedStore({
      [PEER]: { peerId: PEER, roleId: "ally", grantedAt: 1, note: "" },
    });
    rerender(<FriendRoleBadge peerId={PEER} />);
    expect(screen.getByTestId("contact-friend-role-" + PEER)).toBeTruthy();
    expect(screen.getByTestId("contact-friend-role-" + PEER).textContent).toContain("盟友");
  });

  it("已过期绑定灰显为过期徽章（判定面按 Expired 拒）", () => {
    seedStore({
      [PEER]: {
        peerId: PEER,
        roleId: "ally",
        grantedAt: 1,
        note: "",
        expiresAt: Math.floor(Date.now() / 1000) - 10,
      },
    });
    render(<FriendRoleBadge peerId={PEER} />);
    const badge = screen.getByTestId("contact-friend-role-" + PEER);
    expect(badge.textContent).not.toContain("盟友");
  });

  it("未绑定占位提示仅无绑定时渲染", () => {
    const { rerender } = render(<RoleUnboundHint peerId={PEER} />);
    expect(screen.getByTestId("contact-friend-role-none-" + PEER)).toBeTruthy();
    seedStore({
      [PEER]: { peerId: PEER, roleId: "ally", grantedAt: 1, note: "" },
    });
    rerender(<RoleUnboundHint peerId={PEER} />);
    expect(screen.queryByTestId("contact-friend-role-none-" + PEER)).toBeNull();
  });
});

describe("FriendRoleDialog 三态", () => {
  // Harness 复刻父组件语义：onOpenChange(false) 即从树上摘除对话框
  //（chat-friend-remove.test 先例）。
  function DialogHarness({ friend }: { friend: ChatFriendJson }) {
    const [open, setOpen] = useState(true);
    return open ? (
      <FriendRoleDialog friend={friend} onOpenChange={(o) => !o && setOpen(false)} />
    ) : null;
  }

  async function pickRole(name: string): Promise<void> {
    fireEvent.click(screen.getByTestId("friend-role-select"));
    const option = await screen.findByRole("option", { name: new RegExp(name) });
    fireEvent.click(option, { button: 0, pointerType: "mouse" });
  }

  it("绑定成功：选角色后提交调用 authz_bind 并关闭", async () => {
    render(<DialogHarness friend={friendOf(PEER)} />);
    await pickRole("盟友");
    fireEvent.click(screen.getByTestId("friend-role-submit"));
    await waitFor(() => expect(mocks.authzBind).toHaveBeenCalledTimes(1));
    expect(mocks.authzBind).toHaveBeenCalledWith(PEER, "ally", null, null);
    await waitFor(() =>
      expect(screen.queryByTestId("friend-role-dialog")).toBeNull(),
    );
  });

  it("解绑：已绑定好友经解绑按钮调用 authz_unbind", async () => {
    seedStore({
      [PEER]: { peerId: PEER, roleId: "ally", grantedAt: 1, note: "" },
    });
    render(<DialogHarness friend={friendOf(PEER)} />);
    await waitFor(() => expect(screen.getByTestId("friend-role-unbind")).toBeTruthy());
    fireEvent.click(screen.getByTestId("friend-role-unbind"));
    await waitFor(() => expect(mocks.authzUnbind).toHaveBeenCalledWith(PEER));
  });

  it("校验失败：自定义过期填非法值原位报错且不发起命令", async () => {
    render(<DialogHarness friend={friendOf(PEER)} />);
    await pickRole("盟友");
    fireEvent.click(screen.getByTestId("friend-role-expiry"));
    const custom = await screen.findByRole("option", { name: /Unix/ });
    fireEvent.click(custom, { button: 0, pointerType: "mouse" });
    fireEvent.change(screen.getByTestId("friend-role-expiry-custom"), {
      target: { value: "-5" },
    });
    fireEvent.click(screen.getByTestId("friend-role-submit"));
    await waitFor(() =>
      expect(screen.getByTestId("friend-role-error")).toBeTruthy(),
    );
    expect(mocks.authzBind).not.toHaveBeenCalled();
  });
});
