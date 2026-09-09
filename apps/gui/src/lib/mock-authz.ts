import type {
  AuthzBindingJson,
  AuthzBindReport,
  AuthzCheckReport,
  AuthzDefaultRoleReport,
  AuthzRoleView,
  IpcBackend,
} from "./ipc-types";

// 契约 §18 命令面 mock（authz S3）：同签名独立文件（mock-chat 先例）。
// 内存态模拟 §7 判定瀑布（无绑定/过期/角色缺失/权限缺失四步拒绝），
// 内建四角色与 §5 阶梯同源；角色创建入口本轮在 CLI，mock 暴露 seed 注入。

// §5 内建四角色（阶梯并集 = §4 闭集九 key，测试锚定）。
const BUILTIN_ROLE_SOURCES: Array<
  Pick<AuthzRoleView, "roleId" | "name" | "permissions" | "note">
> = [
  {
    roleId: "friend",
    name: "好友",
    permissions: ["chat.send", "chat.attachment", "a2a.discover"],
    note: "基础社交；加好友默认绑定",
  },
  {
    roleId: "guest",
    name: "访客",
    permissions: ["chat.send", "chat.attachment", "a2a.discover", "acp.session"],
    note: "能看能问 agent",
  },
  {
    roleId: "operator",
    name: "操作员",
    permissions: [
      "chat.send",
      "chat.attachment",
      "a2a.discover",
      "acp.session",
      "a2a.invoke",
      "acp.execute",
      "repair.diag",
    ],
    note: "能操作 agent、跑诊断",
  },
  {
    roleId: "ally",
    name: "盟友",
    permissions: [
      "chat.send",
      "chat.attachment",
      "a2a.discover",
      "acp.session",
      "a2a.invoke",
      "acp.execute",
      "repair.diag",
      "llm.borrow",
      "repair.fix",
    ],
    note: "全面信任；可借额度、可修机器",
  },
];

// §4 闭集九 key 恰为内建阶梯并集（改阶梯即改闭集，与 p2p-authz 数据一致性同思路）。
const PERMISSION_REGISTRY: ReadonlySet<string> = new Set(
  BUILTIN_ROLE_SOURCES.flatMap((r) => r.permissions),
);

const PEER_RE = /^[1-9A-HJ-NP-Za-km-z]{43,44}$/;
const DAY_SECS = 86_400;

function builtinRoles(): AuthzRoleView[] {
  return BUILTIN_ROLE_SOURCES.map((r) => ({ ...r, builtin: true }));
}

function requirePeer(peerId: string): void {
  if (!PEER_RE.test(peerId)) {
    throw new Error(`peerId 非法（应为 base58 的 32 字节 PeerId）: ${peerId}`);
  }
}

interface MockAuthzState {
  bindings: Map<string, AuthzBindingJson>;
  customRoles: AuthzRoleView[];
  defaultRoleId: string;
}

const state: MockAuthzState = {
  bindings: new Map(),
  customRoles: [],
  defaultRoleId: "friend",
};

function allRoles(): AuthzRoleView[] {
  return [...builtinRoles(), ...state.customRoles];
}

function nowUnix(): number {
  return Math.floor(Date.now() / 1000);
}

function deny(
  peerId: string,
  permission: string,
  reason: AuthzCheckReport["reason"],
): AuthzCheckReport {
  return { peerId, permission, decision: "deny", reason };
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

export type AuthzIpcMethods = Pick<
  IpcBackend,
  | "authzRoleList"
  | "authzBindingsList"
  | "authzBind"
  | "authzUnbind"
  | "authzCheck"
  | "authzDefaultRoleGet"
  | "authzDefaultRoleSave"
>;

export const mockAuthzBackend: AuthzIpcMethods = {
  async authzRoleList() {
    await delay(80);
    return { roles: allRoles() };
  },

  async authzBindingsList() {
    await delay(80);
    return { bindings: [...state.bindings.values()].map((b) => ({ ...b })) };
  },

  async authzBind(peerId, roleId, expiresAt, note) {
    requirePeer(peerId);
    await delay(120);
    const role = allRoles().find((r) => r.roleId === roleId);
    if (!role) throw new Error(`角色不存在: ${roleId}（内建四或已创建的自定义角色）`);
    const existing = state.bindings.get(peerId);
    const report: AuthzBindReport = {
      peerId,
      roleId,
      created: !existing,
      grantedAt: existing?.grantedAt ?? nowUnix(),
      expiresAt: expiresAt ?? undefined,
      note: note ?? "",
    };
    state.bindings.set(peerId, { ...report });
    return { ...report };
  },

  async authzUnbind(peerId) {
    requirePeer(peerId);
    await delay(120);
    const removed = state.bindings.get(peerId);
    if (!removed) throw new Error(`该好友无绑定: ${peerId}`);
    state.bindings.delete(peerId);
    return { peerId, roleId: removed.roleId };
  },

  async authzCheck(peerId, permission) {
    requirePeer(peerId);
    await delay(60);
    if (!PERMISSION_REGISTRY.has(permission)) {
      throw new Error(
        `权限 key 不在闭集登记表: ${permission}（可用: ${[...PERMISSION_REGISTRY].join(", ")}）`,
      );
    }
    const binding = state.bindings.get(peerId);
    if (!binding) return deny(peerId, permission, "NotBound");
    if (binding.expiresAt !== undefined && binding.expiresAt <= nowUnix()) {
      return deny(peerId, permission, "Expired");
    }
    const role = allRoles().find((r) => r.roleId === binding.roleId);
    if (!role) return deny(peerId, permission, "BrokenRole");
    if (!role.permissions.includes(permission)) {
      return deny(peerId, permission, "MissingPerm");
    }
    return { peerId, permission, decision: "allow", reason: null };
  },

  async authzDefaultRoleGet(): Promise<AuthzDefaultRoleReport> {
    await delay(40);
    return { roleId: state.defaultRoleId };
  },

  async authzDefaultRoleSave(roleId): Promise<AuthzDefaultRoleReport> {
    await delay(80);
    if (roleId !== "" && !allRoles().some((r) => r.roleId === roleId)) {
      throw new Error(`角色未登记，无法设为默认: ${roleId}`);
    }
    state.defaultRoleId = roleId;
    return { roleId };
  },
};

// dev 注入入口：演示/测试播种绑定与自定义角色（window.__MOCK_AUTHZ__）。
export const mockAuthzController = {
  seedBinding(peerId: string, roleId: string, expiresInDays?: number): void {
    requirePeer(peerId);
    state.bindings.set(peerId, {
      peerId,
      roleId,
      grantedAt: nowUnix(),
      note: "seeded",
      expiresAt:
        expiresInDays !== undefined
          ? nowUnix() + expiresInDays * DAY_SECS
          : undefined,
    });
  },
  seedCustomRole(role: AuthzRoleView): void {
    state.customRoles = [
      ...state.customRoles.filter((r) => r.roleId !== role.roleId),
      role,
    ];
  },
  setDefaultRole(roleId: string): void {
    state.defaultRoleId = roleId;
  },
  reset(): void {
    state.bindings.clear();
    state.customRoles = [];
    state.defaultRoleId = "friend";
  },
};
