// admin 端点候选选取（§8：agent admin 端点在既有 endpoint 管理处登记）。
// 草稿优先、按 URL 去重；纯逻辑便于测试与弹层/管理卡共用。
import type { AcpEndpoint } from "./protocol";

export interface AdminEndpointRef {
  /** 以 URL 为身份：同地址不同别名视为同端点 */
  id: string;
  label: string;
  url: string;
  token: string;
}

export function adminEndpointCandidates(
  saved: AcpEndpoint[],
  draft: AcpEndpoint,
): AdminEndpointRef[] {
  const out: AdminEndpointRef[] = [];
  const seen = new Set<string>();
  for (const ep of [draft, ...saved]) {
    const url = ep.adminUrl?.trim();
    if (!url || seen.has(url)) continue;
    seen.add(url);
    out.push({
      id: url,
      label: ep.alias?.trim() || url,
      url,
      token: ep.adminToken?.trim() ?? "",
    });
  }
  return out;
}
