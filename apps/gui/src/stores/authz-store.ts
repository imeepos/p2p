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
  bindings: {},
  defaultRoleId: "friend",
  loadError: null,

  loadAll: async () => {
    try {
      const [{ roles }, { bindings }, { roleId }] = await Promise.all([
        ipc.authzRoleList(),
        ipc.authzBindingsList(),
        ipc.authzDefaultRoleGet(),
      ]);
      set({ roles, bindings: indexBindings(bindings), defaultRoleId: roleId, loadError: null });
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

  reset: () => set({ roles: [], bindings: {}, defaultRoleId: "friend", loadError: null }),
}));
