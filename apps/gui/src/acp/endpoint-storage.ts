// ACP 端点草稿/收藏的 localStorage 存取；损坏时显式告警并回空白。
import type { AcpEndpoint } from "./protocol";

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

export function loadStored(): StoredEndpoints {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<StoredEndpoints>;
      return { draft: parsed.draft ?? EMPTY_DRAFT, saved: parsed.saved ?? [] };
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
