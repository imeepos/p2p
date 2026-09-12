import { beforeEach, describe, expect, it } from "vitest";

import { mockAuthzBackend, mockAuthzController } from "@/lib/mock-authz";

// 真实 mock-authz 冒烟（vi.mock 不参与）：管理面四命令校验与后端语义对齐，
// 测试才可信（peer/base58 约束不在本面，创建流不含 peer 参数）。

const CUSTOM = {
  roleId: "tester",
  name: "测试员",
  permissions: ["chat.send", "llm.borrow"],
  builtin: false,
  note: "",
};

beforeEach(() => {
  mockAuthzController.reset();
});

describe("mock authzPermissionsList", () => {
  it("返回闭集九 key", async () => {
    const { permissions } = await mockAuthzBackend.authzPermissionsList();
    expect(permissions).toHaveLength(9);
    expect(new Set(permissions)).toEqual(
      new Set([
        "chat.send",
        "chat.attachment",
        "a2a.discover",
        "a2a.invoke",
        "acp.session",
        "acp.execute",
        "llm.borrow",
        "repair.diag",
        "repair.fix",
      ]),
    );
  });
});

describe("mock authzRoleCreate", () => {
  it("成功创建并出现在角色清单", async () => {
    const { role } = await mockAuthzBackend.authzRoleCreate(
      CUSTOM.roleId,
      CUSTOM.name,
      CUSTOM.permissions,
      "",
    );
    expect(role.roleId).toBe("tester");
    const { roles } = await mockAuthzBackend.authzRoleList();
    expect(roles.some((r) => r.roleId === "tester" && !r.builtin)).toBe(true);
  });

  it("roleId 非法格式抛可读错误", async () => {
    await expect(
      mockAuthzBackend.authzRoleCreate("Bad_ID", "x", ["chat.send"], ""),
    ).rejects.toThrow(/a-z0-9/);
  });

  it("与内建角色冲突即拒", async () => {
    await expect(
      mockAuthzBackend.authzRoleCreate("friend", "x", ["chat.send"], ""),
    ).rejects.toThrow("冲突");
  });

  it("表外权限 key 即拒", async () => {
    await expect(
      mockAuthzBackend.authzRoleCreate("tester", "x", ["nope.dot"], ""),
    ).rejects.toThrow("闭集");
  });
});

describe("mock authzRoleUpdate", () => {
  it("改自定义角色权限集（保序去重）", async () => {
    mockAuthzController.seedCustomRole({ ...CUSTOM });
    const { role } = await mockAuthzBackend.authzRoleUpdate(
      "tester",
      "测试员二号",
      ["llm.borrow", "chat.send", "llm.borrow"],
      "",
    );
    expect(role.name).toBe("测试员二号");
    expect(role.permissions).toEqual(["llm.borrow", "chat.send"]);
  });

  it("拒改内建角色；未登记角色报不存在", async () => {
    await expect(
      mockAuthzBackend.authzRoleUpdate("ally", "x", ["chat.send"], ""),
    ).rejects.toThrow("内建角色不可修改");
    await expect(
      mockAuthzBackend.authzRoleUpdate("ghost", "x", ["chat.send"], ""),
    ).rejects.toThrow("不存在");
  });
});

describe("mock authzRoleDelete", () => {
  it("无引用自定义角色可删；有绑定引用即拒", async () => {
    mockAuthzController.seedCustomRole({ ...CUSTOM });
    await mockAuthzBackend.authzRoleDelete("tester");
    const { roles } = await mockAuthzBackend.authzRoleList();
    expect(roles.some((r) => r.roleId === "tester")).toBe(false);

    mockAuthzController.seedCustomRole({ ...CUSTOM });
    mockAuthzController.seedBinding(
      "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX",
      "tester",
    );
    await expect(mockAuthzBackend.authzRoleDelete("tester")).rejects.toThrow("绑定引用");
  });

  it("拒删内建角色；默认角色拒删（防悬空）", async () => {
    await expect(mockAuthzBackend.authzRoleDelete("friend")).rejects.toThrow("内建角色不可删除");
    mockAuthzController.seedCustomRole({ ...CUSTOM });
    mockAuthzController.setDefaultRole("tester");
    await expect(mockAuthzBackend.authzRoleDelete("tester")).rejects.toThrow("默认角色");
  });
});
