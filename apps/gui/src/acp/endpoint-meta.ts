// endpoint 本地元数据（app-shell-redesign §3.2/§3.3）：停用位、连接测试
// 结论与权限策略，按 endpointId 键控，localStorage 存档语义沿用
// endpoint-storage（损坏显式告警回默认，不静默吞）。
import { create } from "zustand";

import { emptyPolicy, type EndpointPolicy } from "./endpoint-policy";

const META_KEY = "p2p-gui-acp-endpoint-meta";

export type EndpointTestOutcome = "ok" | "failed";

interface EndpointMetaState {
  /** 停用位：true = 保留配置、断开且不可发起（§3.3 危险区） */
  disabled: Record<string, boolean>;
  /** 最近一次连接测试结论（§3.2：未通过可保存但显警告徽标） */
  lastTest: Record<string, EndpointTestOutcome>;
  policies: Record<string, EndpointPolicy>;
  setDisabled: (endpointId: string, value: boolean) => void;
  recordTest: (endpointId: string, outcome: EndpointTestOutcome) => void;
  setPolicy: (endpointId: string, policy: EndpointPolicy) => void;
  /** 删除 endpoint 时同步清元数据，防孤儿键累积 */
  forget: (endpointId: string) => void;
  /** 测试专用：清空回空白档（单例 store 的用例隔离入口） */
  resetForTest: () => void;
}

interface PersistedMeta {
  disabled?: Record<string, boolean>;
  lastTest?: Record<string, EndpointTestOutcome>;
  policies?: Record<string, EndpointPolicy>;
}

function loadPersisted(): PersistedMeta {
  try {
    const raw = localStorage.getItem(META_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as PersistedMeta;
      return {
        disabled: parsed.disabled ?? {},
        lastTest: parsed.lastTest ?? {},
        policies: parsed.policies ?? {},
      };
    }
  } catch (error) {
    console.warn("[acp] endpoint 元数据存档不可读，使用空白档", error);
  }
  return {};
}

function persist(state: EndpointMetaState): void {
  try {
    const payload: PersistedMeta = {
      disabled: state.disabled,
      lastTest: state.lastTest,
      policies: state.policies,
    };
    localStorage.setItem(META_KEY, JSON.stringify(payload));
  } catch (error) {
    console.warn("[acp] endpoint 元数据存档不可写，仅本次生效", error);
  }
}

const initial = loadPersisted();

export const useEndpointMetaStore = create<EndpointMetaState>()((set, get) => ({
  disabled: initial.disabled ?? {},
  lastTest: initial.lastTest ?? {},
  policies: initial.policies ?? {},

  setDisabled: (endpointId, value) => {
    set((s) => ({ disabled: { ...s.disabled, [endpointId]: value } }));
    persist(get());
  },

  recordTest: (endpointId, outcome) => {
    set((s) => ({ lastTest: { ...s.lastTest, [endpointId]: outcome } }));
    persist(get());
  },

  setPolicy: (endpointId, policy) => {
    set((s) => ({ policies: { ...s.policies, [endpointId]: policy } }));
    persist(get());
  },

  forget: (endpointId) => {
    set((s) => {
      const disabled = { ...s.disabled };
      const lastTest = { ...s.lastTest };
      const policies = { ...s.policies };
      delete disabled[endpointId];
      delete lastTest[endpointId];
      delete policies[endpointId];
      return { disabled, lastTest, policies };
    });
    persist(get());
  },

  resetForTest: () => {
    set({ disabled: {}, lastTest: {}, policies: {} });
    persist(get());
  },
}));

/** 策略读取兜底：未配置过策略的 endpoint 返回空白策略（全部 ask） */
export function policyOf(endpointId: string | null): EndpointPolicy {
  if (!endpointId) return emptyPolicy();
  return useEndpointMetaStore.getState().policies[endpointId] ?? emptyPolicy();
}

/** 删除 endpoint 的元数据清场（联系人 agent 区删除动作复用） */
export function forgetEndpointMeta(endpointId: string): void {
  useEndpointMetaStore.getState().forget(endpointId);
}

/** 测试结论登记（use-endpoint-test 共用入口） */
export function recordTestOutcome(endpointId: string, outcome: EndpointTestOutcome): void {
  useEndpointMetaStore.getState().recordTest(endpointId, outcome);
}
