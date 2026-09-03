import { create } from "zustand";

import { ipc } from "@/lib/ipc";
import type { NodeProfile } from "@/lib/ipc-types";

export const EMPTY_PROFILE: NodeProfile = {
  name: "",
  description: "",
  avatar: null,
};

export interface ProfileStoreState {
  profile: NodeProfile;
  loaded: boolean;
  loadError: string | null;
  load: () => Promise<void>;
  save: (next: NodeProfile) => Promise<NodeProfile>;
}

// 节点资料 store：与节点运行态无关，启动加载一次，保存后本地同步。
export const useProfileStore = create<ProfileStoreState>()((set) => ({
  profile: EMPTY_PROFILE,
  loaded: false,
  loadError: null,

  load: async () => {
    try {
      const profile = await ipc.profileGet();
      set({ profile, loaded: true, loadError: null });
    } catch (error) {
      // 失败留信号：console + loadError 双通道；消费方按 loadError 出重试。
      console.error("[profile-store] profile_get 失败", error);
      set({ loadError: error instanceof Error ? error.message : String(error) });
      throw error;
    }
  },

  save: async (next) => {
    const profile = await ipc.profileSave(next);
    set({ profile, loaded: true });
    return profile;
  },
}));
