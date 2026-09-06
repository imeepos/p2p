// agent 本地管理 HTTP 客户端（docs/design/acp-share-design.md §5）：
// 只打 127.0.0.1 admin 面，Bearer 鉴权；POST /shares 创建（响应含 token 原文与
// 链接要素）、GET /shares 列表（脱敏）、DELETE /shares/{id} 撤销。
// 响应一律容错解析：非 2xx 抛带状态码的错误，由 UI 层映射文案。
import type { ShareEntry, ShareLinkParts } from "./share-model";
import { buildShareLink } from "./share-model";

export interface ShareCreateResponse {
  shareId: string;
  /** token 原文：只出现在创建响应与链接里一次，绝不落日志 */
  token: string;
  link: string;
  expiresAtUnix: number | null;
}

async function adminJson(
  url: string,
  token: string,
  init?: RequestInit,
): Promise<Record<string, unknown>> {
  const res = await fetch(url, {
    ...init,
    headers: {
      Authorization: "Bearer " + token,
      "Content-Type": "application/json",
      ...(init?.headers ?? {}),
    },
  });
  const body = (await res.json().catch(() => null)) as Record<string, unknown> | null;
  if (!res.ok) {
    const detail = typeof body?.error === "string" ? body.error : "";
    throw new Error("HTTP " + res.status + (detail ? ": " + detail : ""));
  }
  return body ?? {};
}

function asString(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function asNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function asStringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((a): a is string => typeof a === "string" && a !== "")
    : [];
}

/** 创建分享：agent 已回传 link 则直接用，否则按链接要素本地拼装（§5） */
export async function createShare(
  adminUrl: string,
  adminToken: string,
  body: Record<string, unknown>,
): Promise<ShareCreateResponse> {
  const url = adminUrl.replace(/\/+$/, "") + "/shares";
  const data = await adminJson(url, adminToken, {
    method: "POST",
    body: JSON.stringify(body),
  });
  const shareId = asString(data.share_id);
  const token = asString(data.token);
  if (!shareId || !token) throw new Error("share create response incomplete");
  const parts: ShareLinkParts = {
    peer: asString(data.peer),
    addrs: asStringArray(data.addrs),
    token,
    expUnix: asNumber(data.expires_at_unix),
    sid: shareId,
  };
  const link = asString(data.link) || (parts.peer ? buildShareLink(parts) : "");
  if (!link) throw new Error("share create response missing link elements");
  return { shareId, token, link, expiresAtUnix: parts.expUnix };
}

function toEntry(item: unknown): ShareEntry | null {
  if (!item || typeof item !== "object") return null;
  const r = item as Record<string, unknown>;
  const shareId = asString(r.share_id);
  if (!shareId) return null;
  const scope = r.scope === "workspace" ? "workspace" : "sandbox";
  return {
    share_id: shareId,
    scope,
    allow_mcp: asStringArray(r.allow_mcp),
    max_activations: asNumber(r.max_activations) ?? 1,
    activations: asNumber(r.activations) ?? 0,
    expires_at_unix: asNumber(r.expires_at_unix) ?? 0,
    revoked: r.revoked === true,
    note: asString(r.note),
    created_at: asString(r.created_at),
    bound_peer: asString(r.bound_peer) || null,
  };
}

/** 分享台账列表；旧 agent 返回裸数组、新壳返回 {shares:[...]}，两者都收 */
export async function listShares(
  adminUrl: string,
  adminToken: string,
): Promise<ShareEntry[]> {
  const url = adminUrl.replace(/\/+$/, "") + "/shares";
  const data = await adminJson(url, adminToken);
  const raw = Array.isArray(data.shares) ? data.shares : Array.isArray(data) ? data : [];
  return raw.map(toEntry).filter((e): e is ShareEntry => e !== null);
}

/** 撤销分享（§3 级联语义由 agent 侧执行，GUI 只触发） */
export async function revokeShare(
  adminUrl: string,
  adminToken: string,
  shareId: string,
): Promise<void> {
  const url = adminUrl.replace(/\/+$/, "") + "/shares/" + encodeURIComponent(shareId);
  await adminJson(url, adminToken, { method: "DELETE" });
}
