import { create } from "zustand";

import { ipc } from "@/lib/ipc";
import type {
  AuthzBindingJson,
  AuthzRoleView,
} from "@/lib/ipc-types";

// 好友角色状态（契约 §18，authz S3）：角色清单 + 按 peerId 索引的当前绑定 +
// 加好友默认角色。通讯录页挂载即拉取；绑定/解绑成功后本地同步推进，
// 失败原样上抛由调用面 toast 呈现（不静默吞）。

export interface AuthzStoreState {
  roles: AuthzRoleView[];
  /** §4 权限闭集 key（authz_permissions_list），角色管理复选框数据源 */
  permissions: string[];
  /** 全量绑定（peer 级单值），按 peerId 索引供好友行 O(1) 徽章取值 */
  bindings: Record<string, AuthzBindingJson>;
  defaultRoleId: string;
  /** 任一清单首次加载是否失败（true = 好友页可提示重试） */
  loadError: string | null;
  loadAll: () => Promise<void>;
  bindRole: (
    peerId: string,
    roleId: string,
    expiresAt?: number | null,
  ) => Promise<void>;
  unbindRole: (peerId: string) => Promise<void>;
  saveDefaultRole: (roleId: string) => Promise<void>;
  createRole: (
    roleId: string,
    name: string,
    permissions: string[],
    note: string,
  ) => Promise<void>;
  updateRole: (
    roleId: string,
    name: string,
    permissions: string[],
    note: string,
  ) => Promise<void>;
  deleteRole: (roleId: string) => Promise<void>;
  /** 测试复位 */
  reset: () => void;
}

function indexBindings(bindings: AuthzBindingJson[]): Record<string, AuthzBindingJson> {
  const indexed: Record<string, AuthzBindingJson> = {};
  for (const binding of bindings) indexed[binding.peerId] = binding;
  return indexed;
}

export const useAuthzStore = create<AuthzStoreState>()((set, get) => ({
  roles: [],
  permissions: [],
  bindings: {},
  defaultRoleId: "friend",
  loadError: null,

  loadAll: async () => {
    try {
      const [{ roles }, { permissions }, { bindings }, { roleId }] = await Promise.all([
        ipc.authzRoleList(),
        ipc.authzPermissionsList(),
        ipc.authzBindingsList(),
        ipc.authzDefaultRoleGet(),
      ]);
      set({
        roles,
        permissions,
        bindings: indexBindings(bindings),
        defaultRoleId: roleId,
        loadError: null,
      });
    } catch (error) {
      // 失败留信号（console 可观测），UI 呈现重试入口，不静默吞
      console.error("[authz] 角色数据加载失败", error);
      set({ loadError: error instanceof Error ? error.message : String(error) });
    }
  },

  bindRole: async (peerId, roleId, expiresAt) => {
    const report = await ipc.authzBind(peerId, roleId, expiresAt ?? null, null);
    set((s) => ({
      bindings: {
        ...s.bindings,
        [peerId]: {
          peerId: report.peerId,
          roleId: report.roleId,
          grantedAt: report.grantedAt,
          note: report.note,
          expiresAt: report.expiresAt,
        },
      },
    }));
  },

  unbindRole: async (peerId) => {
    await ipc.authzUnbind(peerId);
    const next = { ...get().bindings };
    delete next[peerId];
    set({ bindings: next });
  },

  saveDefaultRole: async (roleId) => {
    await ipc.authzDefaultRoleSave(roleId);
    set({ defaultRoleId: roleId });
  },

  createRole: async (roleId, name, permissions, note) => {
    try {
      const { role } = await ipc.authzRoleCreate(roleId, name, permissions, note);
      set((s) => ({ roles: [...s.roles, role] }));
    } catch (error) {
      console.error("[authz] 角色创建失败", error);
      throw error;
    }
  },

  updateRole: async (roleId, name, permissions, note) => {
    try {
      const { role } = await ipc.authzRoleUpdate(roleId, name, permissions, note);
      set((s) => ({ roles: s.roles.map((r) => (r.roleId === role.roleId ? role : r)) }));
    } catch (error) {
      console.error("[authz] 角色更新失败", error);
      throw error;
    }
  },

  deleteRole: async (roleId) => {
    try {
      await ipc.authzRoleDelete(roleId);
    } catch (error) {
      console.error("[authz] 角色删除失败", error);
      throw error;
    }
    set((s) => {
      // defaultRoleId 悬空防御：正常应被后端拒删，本地仍回退 friend 留信号
      if (s.defaultRoleId === roleId) {
        console.error("[authz] 默认角色指向被删角色，本地回退 friend", roleId);
        return {
          roles: s.roles.filter((r) => r.roleId !== roleId),
          defaultRoleId: "friend",
        };
      }
      return { roles: s.roles.filter((r) => r.roleId !== roleId) };
    });
  },

  reset: () =>
    set({ roles: [], permissions: [], bindings: {}, defaultRoleId: "friend", loadError: null }),
}));
