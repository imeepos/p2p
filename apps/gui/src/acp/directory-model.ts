// 连接目录模型：发现清单（console /discovery 契约形状）+ 手动 PeerId + scope。
// scope 默认 sandbox（设计 §12 Q2 拍板）；条目按 peer 幂等去重。

export type AcpScope = "sandbox" | "workspace" | "owner";
export type DirectorySource = "discovered" | "manual";

export interface DirectoryEntry {
  peer: string;
  /** owner 声明名（发现侧可带），无则显示 peer */
  name: string | null;
  scope: AcpScope;
  source: DirectorySource;
  addrs: string[];
}

/** console /discovery 快照条目（apps/acp-console/README.md stdout 契约） */
export interface DiscoveryPeer {
  peer: string;
  addrs?: string[];
}

export const DIRECTORY_SCOPES: AcpScope[] = ["sandbox", "workspace", "owner"];

/** 发现条目上迁：已有条目只补 addrs（scope/来源不降级），新条目默认 sandbox */
export function upsertDiscovered(
  entries: DirectoryEntry[],
  peers: DiscoveryPeer[],
): DirectoryEntry[] {
  const next = entries.map((e) => ({ ...e }));
  for (const peer of peers) {
    if (!peer || typeof peer.peer !== "string" || peer.peer === "") continue;
    const existing = next.find((e) => e.peer === peer.peer);
    const addrs = Array.isArray(peer.addrs) ? peer.addrs.filter((a) => typeof a === "string") : [];
    if (existing) {
      existing.addrs = mergeAddrs(existing.addrs, addrs);
      continue;
    }
    next.push({ peer: peer.peer, name: null, scope: "sandbox", source: "discovered", addrs });
  }
  return next;
}

function mergeAddrs(current: string[], incoming: string[]): string[] {
  return [...new Set([...current, ...incoming])];
}

/** 手动添加：空 peer/重复添加幂等（返回原数组，不触发多余渲染） */
export function addManual(
  entries: DirectoryEntry[],
  peer: string,
): DirectoryEntry[] {
  const trimmed = peer.trim();
  if (trimmed === "") return entries;
  if (entries.some((e) => e.peer === trimmed)) return entries;
  return [
    ...entries,
    { peer: trimmed, name: null, scope: "sandbox", source: "manual", addrs: [] },
  ];
}

export function removeEntry(entries: DirectoryEntry[], peer: string): DirectoryEntry[] {
  return entries.filter((e) => e.peer !== peer);
}

export function setEntryScope(
  entries: DirectoryEntry[],
  peer: string,
  scope: AcpScope,
): DirectoryEntry[] {
  return entries.map((e) => (e.peer === peer ? { ...e, scope } : e));
}