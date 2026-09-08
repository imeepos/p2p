// 本机 agent A2A 管理面客户端（gui-contract §17.2）：
// GET/POST /a2a/agents，PUT/DELETE /a2a/agents/{id}，Bearer 鉴权。
// HTTP 客户端复用 acp/share-admin-client 的 adminJson（同一 admin 管道，禁双源）；
// 非 2xx 抛带原文的 Error，由调用方 toast 原文上浮（禁静默）。
import { adminJson } from "@/acp/share-admin-client";

import type { AgentDefJson } from "./types";

function agentsUrl(adminUrl: string): string {
  return adminUrl.replace(/\/+$/, "") + "/a2a/agents";
}

function toDef(item: unknown): AgentDefJson {
  const r = (item ?? {}) as Record<string, unknown>;
  return {
    agentId: typeof r.agentId === "string" ? r.agentId : "",
    name: typeof r.name === "string" ? r.name : "",
    description: typeof r.description === "string" ? r.description : "",
    skills: Array.isArray(r.skills) ? (r.skills as AgentDefJson["skills"]) : [],
    visibility:
      r.visibility === "public" || r.visibility === "local" ? r.visibility : "private",
    enabled: r.enabled !== false,
    createdAt: typeof r.createdAt === "number" ? r.createdAt : 0,
  };
}

/** 我发布的定义全集；旧 agent（无端点）404 原样上抛，由 store 记可观测降级。 */
export async function listAgents(adminUrl: string, token: string): Promise<AgentDefJson[]> {
  const data = await adminJson(agentsUrl(adminUrl), token);
  const raw = Array.isArray(data.agents) ? data.agents : [];
  return raw.map(toDef).filter((d) => d.agentId !== "");
}

export async function createAgent(
  adminUrl: string,
  token: string,
  body: Record<string, unknown>,
): Promise<AgentDefJson> {
  const data = await adminJson(agentsUrl(adminUrl), token, {
    method: "POST",
    body: JSON.stringify(body),
  });
  return toDef(data);
}

export async function updateAgent(
  adminUrl: string,
  token: string,
  agentId: string,
  patch: Record<string, unknown>,
): Promise<AgentDefJson> {
  const data = await adminJson(
    agentsUrl(adminUrl) + "/" + encodeURIComponent(agentId),
    token,
    { method: "PUT", body: JSON.stringify(patch) },
  );
  return toDef(data);
}

export async function removeAgent(
  adminUrl: string,
  token: string,
  agentId: string,
): Promise<void> {
  await adminJson(agentsUrl(adminUrl) + "/" + encodeURIComponent(agentId), token, {
    method: "DELETE",
  });
}

/** 生成签名邀请帧：POST /a2a/agents/{id}/invite，返回邀请帧 JSON（Signed<InvitePayload>）。 */
export async function createInvite(
  adminUrl: string,
  token: string,
  agentId: string,
  inviteePeer: string,
  expirySecs?: number,
): Promise<Record<string, unknown>> {
  const data = await adminJson(
    agentsUrl(adminUrl) + "/" + encodeURIComponent(agentId) + "/invite",
    token,
    {
      method: "POST",
      body: JSON.stringify({ inviteePeer, expirySecs }),
    },
  );
  return data.invite as Record<string, unknown>;
}
