import { useState } from "react";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AuthzRoleView } from "@/lib/ipc-types";
import { useAuthzStore } from "@/stores/authz-store";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    authzRoleCreate: vi.fn<
      (roleId: string, name: string, permissions: string[], note: string) => Promise<unknown>
    >(),
    authzRoleUpdate: vi.fn<
      (roleId: string, name: string, permissions: string[], note: string) => Promise<unknown>
    >(),
    authzRoleDelete: vi.fn<(roleId: string) => Promise<unknown>>(),
    authzRoleList: vi.fn<() => Promise<{ roles: AuthzRoleView[] }>>(),
    authzPermissionsList: vi.fn<() => Promise<{ permissions: string[] }>>(),
    authzBindingsList: vi.fn<() => Promise<{ bindings: unknown[] }>>(),
    authzDefaultRoleGet: vi.fn<() => Promise<{ roleId: string }>>(),
    authzDefaultRoleSave: vi.fn<(roleId: string) => Promise<{ roleId: string }>>(),
    authzBind: vi.fn<() => Promise<unknown>>(),
    authzUnbind: vi.fn<() => Promise<unknown>>(),
  },
}));

vi.mock("@/lib/ipc", () => ({ ipc: mocks }));

import "@/i18n";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { RoleManagerDialog } from "@/views/contacts/role-manager-dialog";

const BUILTIN: AuthzRoleView[] = [
  { roleId: "friend", name: "好友", permissions: ["chat.send"], builtin: true, note: "" },
  { roleId: "guest", name: "访客", permissions: ["chat.send"], builtin: true, note: "" },
  { roleId: "operator", name: "操作员", permissions: ["chat.send"], builtin: true, note: "" },
  { roleId: "ally", name: "盟友", permissions: ["chat.send"], builtin: true, note: "" },
];

const CUSTOM: AuthzRoleView = {
  roleId: "helper",
  name: "帮手",
  permissions: ["chat.send", "acp.session"],
  builtin: false,
  note: "seeded",
};

const CLOSED_SET = ["chat.send", "chat.attachment", "a2a.discover", "llm.borrow"];

function seedStore(roles: AuthzRoleView[]): void {
  useAuthzStore.setState({
    roles,
    permissions: CLOSED_SET,
    bindings: {},
    defaultRoleId: "friend",
    loadError: null,
  });
}

// Harness 复刻父组件语义：onOpenChange(false) 即从树上摘除对话框。
function DialogHarness() {
  const [open, setOpen] = useState(true);
  return open ? (
    <ConfirmProvider>
      <RoleManagerDialog onOpenChange={(o) => !o && setOpen(false)} />
    </ConfirmProvider>
  ) : null;
}

function openCreateForm(): void {
  fireEvent.click(screen.getByTestId("role-manager-new"));
}

beforeEach(() => {
  mocks.authzRoleCreate.mockReset().mockResolvedValue({});
  mocks.authzRoleUpdate.mockReset().mockResolvedValue({});
  mocks.authzRoleDelete.mockReset().mockResolvedValue({ roleId: "" });
  mocks.authzRoleList.mockReset().mockResolvedValue({ roles: BUILTIN });
  mocks.authzPermissionsList.mockReset().mockResolvedValue({ permissions: CLOSED_SET });
  mocks.authzBindingsList.mockReset().mockResolvedValue({ bindings: [] });
  mocks.authzDefaultRoleGet.mockReset().mockResolvedValue({ roleId: "friend" });
  mocks.authzDefaultRoleSave.mockReset().mockResolvedValue({ roleId: "" });
  seedStore(BUILTIN);
});

describe("RoleManagerDialog 角色列表", () => {
  it("渲染内建四角色 + seed 自定义角色", () => {
    seedStore([...BUILTIN, CUSTOM]);
    render(<DialogHarness />);
    for (const role of [...BUILTIN, CUSTOM]) {
      expect(screen.getByTestId("role-row-" + role.roleId)).toBeTruthy();
    }
    expect(screen.getByTestId("role-perms-helper").textContent).toContain("ACP 会话");
  });

  it("builtin 行无编辑/删除钮，自定义行有", () => {
    seedStore([...BUILTIN, CUSTOM]);
    render(<DialogHarness />);
    for (const role of BUILTIN) {
      expect(screen.queryByTestId("role-edit-" + role.roleId)).toBeNull();
      expect(screen.queryByTestId("role-delete-" + role.roleId)).toBeNull();
    }
    expect(screen.getByTestId("role-edit-helper")).toBeTruthy();
    expect(screen.getByTestId("role-delete-helper")).toBeTruthy();
  });
});

describe("RoleManagerDialog 创建流", () => {
  it("填表单勾权限保存后列表出现新角色", async () => {
    const created: AuthzRoleView = {
      roleId: "tester",
      name: "测试员",
      permissions: ["chat.send", "llm.borrow"],
      builtin: false,
      note: "",
    };
    mocks.authzRoleCreate.mockResolvedValue({ role: created });
    render(<DialogHarness />);
    openCreateForm();
    fireEvent.change(screen.getByTestId("role-manager-role-id"), { target: { value: "tester" } });
    fireEvent.change(screen.getByTestId("role-manager-name"), { target: { value: "测试员" } });
    fireEvent.click(screen.getByTestId("role-perm-chat.send"));
    fireEvent.click(screen.getByTestId("role-perm-llm.borrow"));
    fireEvent.click(screen.getByTestId("role-manager-save"));
    await waitFor(() => expect(mocks.authzRoleCreate).toHaveBeenCalledTimes(1));    expect(mocks.authzRoleCreate).toHaveBeenCalledWith(
      "tester",
      "测试员",
      ["chat.send", "llm.borrow"],
      "",
    );
    await waitFor(() => expect(screen.getByTestId("role-row-tester")).toBeTruthy());
  });

  it("校验三态：roleId 非法 / name 空 / 权限零项，均不发起命令", () => {
    render(<DialogHarness />);
    openCreateForm();
    fireEvent.click(screen.getByTestId("role-manager-save"));
    expect(screen.getByTestId("role-manager-error").textContent).toContain("1-32");
    expect(mocks.authzRoleCreate).not.toHaveBeenCalled();

    fireEvent.change(screen.getByTestId("role-manager-role-id"), { target: { value: "Tester!" } });
    fireEvent.click(screen.getByTestId("role-manager-save"));
    expect(screen.getByTestId("role-manager-error")).toBeTruthy();
    expect(mocks.authzRoleCreate).not.toHaveBeenCalled();

    fireEvent.change(screen.getByTestId("role-manager-role-id"), { target: { value: "tester" } });
    fireEvent.change(screen.getByTestId("role-manager-name"), { target: { value: "测试员" } });
    fireEvent.click(screen.getByTestId("role-manager-save"));
    expect(screen.getByTestId("role-manager-error").textContent).toContain("至少勾选");
    expect(mocks.authzRoleCreate).not.toHaveBeenCalled();
  });

  it("命令失败：原位呈现后端错误且不关表单", async () => {
    mocks.authzRoleCreate.mockRejectedValue(new Error("角色 ID 与既有角色冲突: tester"));
    render(<DialogHarness />);
    openCreateForm();
    fireEvent.change(screen.getByTestId("role-manager-role-id"), { target: { value: "tester" } });
    fireEvent.change(screen.getByTestId("role-manager-name"), { target: { value: "测试员" } });
    fireEvent.click(screen.getByTestId("role-perm-chat.send"));
    fireEvent.click(screen.getByTestId("role-manager-save"));
    await waitFor(() =>
      expect(screen.getByTestId("role-manager-error").textContent).toContain("冲突"),
    );
    expect(screen.getByTestId("role-manager-form")).toBeTruthy();
  });
});

describe("RoleManagerDialog 编辑/删除自定义角色", () => {
  it("编辑改权限集后保存生效", async () => {
    const updated: AuthzRoleView = { ...CUSTOM, permissions: ["llm.borrow"] };
    mocks.authzRoleUpdate.mockResolvedValue({ role: updated });
    seedStore([...BUILTIN, CUSTOM]);
    render(<DialogHarness />);
    fireEvent.click(screen.getByTestId("role-edit-helper"));
    expect((screen.getByTestId("role-manager-role-id") as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByTestId("role-perm-chat.send") as HTMLInputElement).checked).toBe(true);
    fireEvent.click(screen.getByTestId("role-perm-chat.send"));
    fireEvent.click(screen.getByTestId("role-perm-llm.borrow"));
    fireEvent.click(screen.getByTestId("role-manager-save"));
    await waitFor(() => expect(mocks.authzRoleUpdate).toHaveBeenCalledWith(
      "helper",
      "帮手",
      ["acp.session", "llm.borrow"],
      "seeded",
    ));
    await waitFor(() =>
      expect(useAuthzStore.getState().roles.find((r) => r.roleId === "helper")?.permissions).toEqual([
        "llm.borrow",
      ]),
    );
  });

  it("删除经 confirm 确认后从列表消失", async () => {
    mocks.authzRoleDelete.mockResolvedValue({ roleId: "helper" });
    seedStore([...BUILTIN, CUSTOM]);
    render(<DialogHarness />);
    fireEvent.click(screen.getByTestId("role-delete-helper"));
    const confirmBox = await screen.findByRole("alertdialog");
    fireEvent.click(within(confirmBox).getByRole("button", { name: "删除" }));
    await waitFor(() => expect(mocks.authzRoleDelete).toHaveBeenCalledWith("helper"));
    await waitFor(() => expect(screen.queryByTestId("role-row-helper")).toBeNull());
  });
});
