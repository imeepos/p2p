// console 本地 status HTTP 客户端（apps/acp-console/README.md status 端点契约）：
// /reattach 续连票据查询与 /discovery 发现清单拉取。Bearer 鉴权；响应一律容错
// 解析——console 不可达/坏形响应折化为明确的 unavailable/空清单，绝不抛出打断重连。
import type { AcpConsoleStatus } from "@/lib/ipc-types";
import type { AcpEndpoint } from "./protocol";
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

/** POST /connect-share 结果（§7）：失败折化为 ok=false + denied 码/原因 */
export interface ConnectShareOutcome {
  ok: boolean;
  peer: string | null;
  scope: string | null;
  code: string | null;
  reason: string | null;
}

// 契约 v10 §15（UX3）：本机 agent 端点自动登记面。稳定本地 id 独立于 status
// 相位重放；peer 不取自 status（契约无该字段），由 console 发现面解析后回填。
export const LOCAL_AGENT_ENDPOINT_ID = "acp-local-agent";

/** ready 状态 -> 本机 agent 端点草稿；非 ready 或缺连接面返回 null */
export function localAgentEndpointOf(status: AcpConsoleStatus | null): AcpEndpoint | null {
  if (!status || status.phase !== "ready" || !status.wsUrl || !status.token) return null;
  return {
    endpointId: LOCAL_AGENT_ENDPOINT_ID,
    wsUrl: status.wsUrl,
    token: status.token,
    peer: "",
    statusUrl: status.statusUrl,
    adminUrl: status.adminUrl,
  };
}

export interface LocalAgentMerge {
  saved: AcpEndpoint[];
  endpoint: AcpEndpoint;
  /** 连接面是否变化（wsUrl/token/statusUrl/adminUrl 任一变化或新登记） */
  changed: boolean;
}

/** 幂等登记：无则追加；有则仅覆盖连接面（alias 空、peer 用户值保留，不产生重复条目） */
export function mergeLocalAgent(
  saved: AcpEndpoint[],
  next: AcpEndpoint,
  alias: string,
): LocalAgentMerge {
  const idx = saved.findIndex((e) => e.endpointId === LOCAL_AGENT_ENDPOINT_ID);
  if (idx < 0) {
    const created: AcpEndpoint = { ...next, alias: next.alias?.trim() || alias };
    return { saved: [...saved, created], endpoint: created, changed: true };
  }
  const cur = saved[idx]!;
  const merged: AcpEndpoint = {
    ...cur,
    wsUrl: next.wsUrl,
    token: next.token,
    statusUrl: next.statusUrl ?? cur.statusUrl,
    adminUrl: next.adminUrl ?? cur.adminUrl,
    alias: cur.alias?.trim() ? cur.alias : alias,
  };
  const changed =
    cur.wsUrl !== merged.wsUrl ||
    cur.token !== merged.token ||
    (cur.statusUrl ?? "") !== (merged.statusUrl ?? "") ||
    (cur.adminUrl ?? "") !== (merged.adminUrl ?? "") ||
    !(cur.alias ?? "").trim();
  const out = changed ? saved.map((e, i) => (i === idx ? merged : e)) : saved;
  return { saved: out, endpoint: merged, changed };
}

const DENIED_UNAVAILABLE: ConnectShareOutcome = {
  ok: false,
  peer: null,
  scope: null,
  code: null,
  reason: "unavailable",
};

function outcomeOf(body: Record<string, unknown> | null): ConnectShareOutcome {
  const ok = body === null ? false : body.ok !== false;
  return {
    ok,
    peer: asString(body?.peer),
    scope: asString(body?.scope),
    code: asString(body?.code),
    reason: asString(body?.reason),
  };
}

/** 按分享链接直拨（§7 GUI 入口）：解析与拨号都在 console，GUI 只投递链接原文。
 *  2xx 视为受理成功（ok 默认 true）；非 2xx 取 body 的 code/reason 作为 denied 面。 */
export async function connectShare(
  statusUrl: string,
  token: string,
  link: string,
): Promise<ConnectShareOutcome> {
  const url = statusUrl.replace(/\/+$/, "") + "/connect-share";
  let res: Response;
  try {
    res = await fetch(url, {
      method: "POST",
      headers: { ...bearerHeaders(token), "Content-Type": "application/json" },
      body: JSON.stringify({ link }),
    });
  } catch (error) {
    console.warn("[acp] console connect-share 不可达", url, error);
    return DENIED_UNAVAILABLE;
  }
  const body = (await res.json().catch(() => null)) as Record<string, unknown> | null;
  if (!res.ok) {
    console.warn("[acp] connect-share denied", res.status, body?.code, body?.reason);
    return { ...outcomeOf(body), ok: false };
  }
  return outcomeOf(body);
}
