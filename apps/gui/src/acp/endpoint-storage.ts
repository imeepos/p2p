// ACP 端点草稿/收藏的 localStorage 存取；损坏时显式告警并回空白。
// §2.2：endpointId（本地 UUID）为会话主键、alias 缺省回退 host:port，
// 读取时对存量条目做一次性迁移并回写。
import type { AcpEndpoint } from "./protocol";
import { wsHostOf } from "@/lib/conversation-entry";

const STORAGE_KEY = "p2p-gui-acp-endpoints";

export interface StoredEndpoints {
  draft: AcpEndpoint;
  saved: AcpEndpoint[];
}

export const EMPTY_DRAFT: AcpEndpoint = {
  wsUrl: "ws://127.0.0.1:8787",
  token: "",
  peer: "",
};

export function newEndpointId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  // 可观测回退：randomUUID 缺席（老 WebView）时用随机串，冲突概率可忽略
  console.warn("[acp] crypto.randomUUID 缺席，使用随机串生成 endpointId");
  return "ep-" + Math.random().toString(36).slice(2) + Date.now().toString(36);
}

function migrateEndpoint(endpoint: AcpEndpoint): AcpEndpoint {
  const alias = endpoint.alias?.trim() || wsHostOf(endpoint.wsUrl) || endpoint.peer;
  const endpointId = endpoint.endpointId?.trim() || newEndpointId();
  if (alias === endpoint.alias && endpointId === endpoint.endpointId) return endpoint;
  return { ...endpoint, endpointId, alias };
}

export function loadStored(): StoredEndpoints {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<StoredEndpoints>;
      const draft = migrateEndpoint(parsed.draft ?? EMPTY_DRAFT);
      const saved = (parsed.saved ?? []).map(migrateEndpoint);
      return { draft, saved };
    }
  } catch (error) {
    console.warn("[acp] 端点存档不可读，使用空白表单", error);
  }
  return { draft: { ...EMPTY_DRAFT }, saved: [] };
}

export function persistStored(stored: StoredEndpoints): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(stored));
  } catch (error) {
    console.warn("[acp] 端点存档不可写，仅本次生效", error);
  }
}
