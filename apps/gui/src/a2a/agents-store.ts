// /agents 页状态（gui-contract §17）：发现面 = 卡片事件通道入簿（version 钳制
// + removed 除名）；我的面 = 本机 agent admin CRUD，失败原文上浮（lastActionError
// 供 toast，禁静默）。通道与 CRUD 互不阻塞。
import { create } from "zustand";

import { createAgent, listAgents, removeAgent, updateAgent } from "./admin-client";
import { CardChannel, cardKey, shouldReplace, type CardChannelStatus } from "./card-channel";
import { toSkillJson } from "./skills";
import type { AgentCreateInput, AgentDefJson, DiscoveredAgent } from "./types";

interface AgentsState {
  /** 发现面卡片簿（含本机公开卡；宿主 list 已按可见性过滤）。 */
  discovered: DiscoveredAgent[];
  /** 我发布的定义全集（admin 面）。 */
  mine: AgentDefJson[];
  channelStatus: CardChannelStatus;
  /** 通道 error 帧原文（上浮 UI，不静默）。 */
  channelError: string | null;
  /** admin 动作失败原文（toast 用，动作级，逐次覆盖）。 */
  lastActionError: string | null;
  /** admin 端点不可用降级标记（旧 agent 404 等常态不可达）。 */
  mineUnavailable: boolean;
  connectCards: (wsUrl: string, token: string, hostPeer: string) => void;
  disconnectCards: () => void;
  loadMine: (adminUrl: string, token: string) => Promise<void>;
  createMine: (adminUrl: string, token: string, input: AgentCreateInput) => Promise<boolean>;
  updateVisibility: (
    adminUrl: string,
    token: string,
    agentId: string,
    visibility: AgentDefJson["visibility"],
  ) => Promise<boolean>;
  unpublish: (adminUrl: string, token: string, agentId: string) => Promise<boolean>;
}

/** 非 store 的通道实例（不进渲染状态）。 */
let channel: CardChannel | null = null;

function ingestCards(
  prev: DiscoveredAgent[],
  incoming: DiscoveredAgent[],
): DiscoveredAgent[] {
  const byKey = new Map(prev.map((row) => [cardKey(row.card), row]));
  for (const row of incoming) {
    const key = cardKey(row.card);
    const current = byKey.get(key)?.card;
    if (shouldReplace(current, row.card)) byKey.set(key, row);
  }
  return [...byKey.values()];
}

export const useAgentsStore = create<AgentsState>((set) => ({
  discovered: [],
  mine: [],
  channelStatus: "offline",
  channelError: null,
  lastActionError: null,
  mineUnavailable: false,

  connectCards: (wsUrl, token, hostPeer) => {
    channel?.close();
    channel = new CardChannel({
      onCards: (cards) => set((s) => ({ discovered: ingestCards(s.discovered, cards) })),
      onRemoved: (keys) =>
        set((s) => ({
          discovered: s.discovered.filter((row) => !keys.includes(cardKey(row.card))),
        })),
      onError: (message) => set({ channelError: message }),
      onStatus: (status) => set({ channelStatus: status }),
    });
    channel.connect(wsUrl, token, hostPeer);
  },

  disconnectCards: () => {
    channel?.close();
    channel = null;
    set({ channelStatus: "offline" });
  },

  loadMine: async (adminUrl, token) => {
    try {
      const mine = await listAgents(adminUrl, token);
      set({ mine, mineUnavailable: false, lastActionError: null });
    } catch (error) {
      // 旧 agent 无 a2a admin 端点属常态降级：可观测但不刷错误墙
      console.warn("[a2a] admin 列表不可达", error);
      set({ mineUnavailable: true, mine: [] });
    }
  },

  createMine: async (adminUrl, token, input) => {
    const skills = input.skills
      .map(toSkillJson)
      .filter((s): s is NonNullable<typeof s> => s !== null);
    try {
      const def = await createAgent(adminUrl, token, {
        name: input.name,
        description: input.description,
        skills,
        visibility: input.visibility,
      });
      set((s) => ({ mine: [...s.mine, def], lastActionError: null }));
      return true;
    } catch (error) {
      set({ lastActionError: error instanceof Error ? error.message : String(error) });
      return false;
    }
  },

  updateVisibility: async (adminUrl, token, agentId, visibility) => {
    try {
      const def = await updateAgent(adminUrl, token, agentId, { visibility });
      set((s) => ({
        mine: s.mine.map((d) => (d.agentId === agentId ? def : d)),
        lastActionError: null,
      }));
      return true;
    } catch (error) {
      set({ lastActionError: error instanceof Error ? error.message : String(error) });
      return false;
    }
  },

  unpublish: async (adminUrl, token, agentId) => {
    try {
      await removeAgent(adminUrl, token, agentId);
      set((s) => ({
        mine: s.mine.filter((d) => d.agentId !== agentId),
        lastActionError: null,
      }));
      return true;
    } catch (error) {
      set({ lastActionError: error instanceof Error ? error.message : String(error) });
      return false;
    }
  },

}));
