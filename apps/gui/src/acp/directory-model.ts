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
  /** 发现渠道（mdns / rendezvous / manual，console source 原样）；非发现来源为 null */
  via: string | null;
}

/** console /discovery 快照条目（apps/acp-console/README.md status 契约）。
 *  name/source 为加法面：旧 console 不带时如实留空，缺字段容错。 */
export interface DiscoveryPeer {
  peer: string;
  addrs?: string[];
  name?: string | null;
  source?: string | null;
}

export const DIRECTORY_SCOPES: AcpScope[] = ["sandbox", "workspace", "owner"];

/** 发现条目上迁：已有条目只补 addrs/name/via（scope/来源不降级），新条目默认 sandbox */
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
      existing.name = asName(peer.name) ?? existing.name;
      existing.via = asVia(peer.source) ?? existing.via;
      continue;
    }
    next.push({
      peer: peer.peer,
      name: asName(peer.name),
      scope: "sandbox",
      source: "discovered",
      addrs,
      via: asVia(peer.source),
    });
  }
  return next;
}

function asName(name: unknown): string | null {
  return typeof name === "string" && name !== "" ? name : null;
}

function asVia(source: unknown): string | null {
  return typeof source === "string" && source !== "" ? source : null;
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
    { peer: trimmed, name: null, scope: "sandbox", source: "manual", addrs: [], via: null },
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

/** 目录信息架构：按来源分组呈现（发现 = rendezvous/mDNS；手动/保存 = 本地录入） */
export interface DirectoryGroups {
  discovered: DirectoryEntry[];
  manual: DirectoryEntry[];
}

export function groupBySource(entries: DirectoryEntry[]): DirectoryGroups {
  const groups: DirectoryGroups = { discovered: [], manual: [] };
  for (const entry of entries) {
    groups[entry.source].push(entry);
  }
  return groups;
}