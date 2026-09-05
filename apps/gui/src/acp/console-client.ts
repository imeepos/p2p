// console 本地 status HTTP 客户端（apps/acp-console/README.md status 端点契约）：
// /reattach 续连票据查询与 /discovery 发现清单拉取。Bearer 鉴权；响应一律容错
// 解析——console 不可达/坏形响应折化为明确的 unavailable/空清单，绝不抛出打断重连。
import type { DiscoveryPeer } from "./directory-model";

/** /reattach reason 面（README 契约） */
export type ReattachReason = "ok" | "missing" | "expired" | "unavailable";

export interface ReattachAnswer {
  /** 窗口内可用票据；null = 按 fresh 流程拨号 */
  ticket: string | null;
  expiresAtUnixMs: number | null;
  reason: ReattachReason;
}

const EMPTY_ANSWER: ReattachAnswer = {
  ticket: null,
  expiresAtUnixMs: null,
  reason: "unavailable",
};

function bearerHeaders(token: string): HeadersInit {
  return { Authorization: "Bearer " + token };
}

async function getJson(url: string, token: string): Promise<unknown | null> {
  try {
    const res = await fetch(url, {
      headers: bearerHeaders(token),
      cache: "no-store",
    });
    if (!res.ok) return null;
    return await res.json();
  } catch (error) {
    console.warn("[acp] console status 不可达", url, error);
    return null;
  }
}

function asString(value: unknown): string | null {
  return typeof value === "string" && value !== "" ? value : null;
}

/** 查询该 peer 当前可用的续连票据；任何失败折化为 reason=unavailable（fresh 拨号） */
export async function queryReattachTicket(
  statusUrl: string,
  token: string,
  peer: string,
): Promise<ReattachAnswer> {
  const url = statusUrl.replace(/\/+$/, "") + "/reattach?peer=" + encodeURIComponent(peer);
  const body = (await getJson(url, token)) as Record<string, unknown> | null;
  if (!body) return EMPTY_ANSWER;
  const raw = asString(body.reason);
  const reason: ReattachReason =
    raw === "ok" || raw === "missing" || raw === "expired" ? raw : "unavailable";
  return {
    ticket: asString(body.ticket),
    expiresAtUnixMs:
      typeof body.expires_at_unix_ms === "number" ? body.expires_at_unix_ms : null,
    reason,
  };
}

/** 拉取发现清单；返回 null = console 不可达/坏形响应（调用方停轮询不误报），
 *  空数组 = console 可达但暂无发现（空态正常引导） */
export async function fetchDiscoveryPeers(
  statusUrl: string,
  token: string,
): Promise<DiscoveryPeer[] | null> {
  const url = statusUrl.replace(/\/+$/, "") + "/discovery";
  const body = (await getJson(url, token)) as Record<string, unknown> | null;
  if (!body) return null;
  const peers = Array.isArray(body.peers) ? body.peers : [];
  const out: DiscoveryPeer[] = [];
  for (const item of peers) {
    if (!item || typeof item !== "object") continue;
    const record = item as Record<string, unknown>;
    const peer = asString(record.peer);
    if (!peer) continue;
    out.push({
      peer,
      addrs: Array.isArray(record.addrs)
        ? record.addrs.filter((a): a is string => typeof a === "string")
        : undefined,
      name: asString(record.name),
      source: asString(record.source),
    });
  }
  return out;
}

