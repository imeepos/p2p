import { create } from "zustand";

import { ipc } from "@/lib/ipc";
import type { PeerProfileJson } from "@/lib/ipc-types";

// 对端自报资料缓存（会话内存态）：添加好友弹窗与好友资料卡共用。
// 查询失败（null）与异常都记入 unavailable，避免对同一离线节点反复轰炸；
// 真实网络态随时变化，不持久化、重进页面重查。

export interface PeerProfileStoreState {
  profiles: Record<string, PeerProfileJson>;
  /** 查询无果（不可达/未设置）的 peer：值为失败时间戳，仅作节流 */
  unavailable: Record<string, number>;
  inflight: Partial<Record<string, Promise<PeerProfileJson | null>>>;
  /** 按需拉取；同一 peer 并发请求合并为一次 */
  fetch: (peerId: string) => Promise<PeerProfileJson | null>;
  /** 只读缓存（不触发网络） */
  cached: (peerId: string) => PeerProfileJson | null;
  /** 测试复位 */
  reset: () => void;
}

const UNAVAILABLE_RETRY_MS = 30_000;

export const usePeerProfileStore = create<PeerProfileStoreState>()((set, get) => ({
  profiles: {},
  unavailable: {},
  inflight: {},

  fetch: (peerId) => {
    const cached = get().profiles[peerId];
    if (cached) return Promise.resolve(cached);
    const failedAt = get().unavailable[peerId];
    if (failedAt && Date.now() - failedAt < UNAVAILABLE_RETRY_MS) return Promise.resolve(null);
    const pending = get().inflight[peerId];
    if (pending) return pending;
    const call = (async () => {
      try {
        const profile = await ipc.chatPeerProfile(peerId);
        set((s) => {
          const nextUnavailable = { ...s.unavailable };
          if (profile) delete nextUnavailable[peerId];
          else nextUnavailable[peerId] = Date.now();
          return {
            profiles: profile ? { ...s.profiles, [peerId]: profile } : s.profiles,
            unavailable: nextUnavailable,
            inflight: { ...s.inflight, [peerId]: undefined },
          };
        });
        return profile;
      } catch (error) {
        // 失败留信号：详情见 console；UI 按 unavailable 降级，不弹错
        console.error("[peer-profile] 对端资料查询失败", peerId, error);
        set((s) => ({
          unavailable: { ...s.unavailable, [peerId]: Date.now() },
          inflight: { ...s.inflight, [peerId]: undefined },
        }));
        return null;
      }
    })();
    set((s) => ({ inflight: { ...s.inflight, [peerId]: call } }));
    return call;
  },

  cached: (peerId) => get().profiles[peerId] ?? null,

  reset: () => set({ profiles: {}, unavailable: {}, inflight: {} }),
}));
