// agent endpoint 表单预校验（app-shell-redesign §3.4）：wsUrl 做 URL 格式
// 校验，错误码稳定经 i18n（contacts.endpoint.errors.<code>）；peer 沿用
// base58 32 字节口径；别名同昵称 trim ≤64 口径；token 分径校验——保存路径
// 可空（§3.2 原始「token（可空）」口径，先存后连），连接类动作必填
// （2026-09-07 用户裁决，部分回调 aaa72f2 的收紧）；adminUrl 可选但填了
// 必须合法（分享管理面）。
import { isValidPeerId, MAX_NICKNAME_CHARS } from "@/lib/chat-limits";

export type EndpointFieldError =
  | "targetRequired"
  | "wsUrlRequired"
  | "wsUrlInvalid"
  | "tokenRequired"
  | "peerInvalid"
  | "aliasTooLong"
  | "adminUrlInvalid";

export interface EndpointFormErrors {
  wsUrl?: EndpointFieldError;
  token?: EndpointFieldError;
  peer?: EndpointFieldError;
  alias?: EndpointFieldError;
  adminUrl?: EndpointFieldError;
}

export function isValidWsUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return url.protocol === "ws:" || url.protocol === "wss:";
  } catch {
    return false;
  }
}

export function isValidAdminUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}

/** 从 wsUrl 推导管理地址默认值：ws://host:port -> http://host:port（同机部署起点，端口可改） */
export function defaultAdminUrl(wsUrl: string): string {
  try {
    const url = new URL(wsUrl.trim());
    if (url.protocol !== "ws:" && url.protocol !== "wss:") return "";
    return "http://" + url.host;
  } catch {
    return "";
  }
}

export function hasEndpointFormErrors(errors: EndpointFormErrors): boolean {
  return (
    errors.wsUrl !== undefined ||
    errors.token !== undefined ||
    errors.peer !== undefined ||
    errors.alias !== undefined ||
    errors.adminUrl !== undefined
  );
}

export interface ValidateEndpointOptions {
  /** 连接类动作（测试连接/添加并开始对话）要求 token；保存路径可空（先存后连） */
  requireToken: boolean;
}

export function validateEndpointForm(
  form: {
    wsUrl: string;
    token: string;
    peer: string;
    alias: string;
    adminUrl?: string;
  },
  options: ValidateEndpointOptions,
): EndpointFormErrors {
  const errors: EndpointFormErrors = {};
  const wsUrl = form.wsUrl.trim();
  if (wsUrl.length === 0) errors.wsUrl = "wsUrlRequired";
  else if (!isValidWsUrl(wsUrl)) errors.wsUrl = "wsUrlInvalid";
  if (options.requireToken && form.token.trim().length === 0) errors.token = "tokenRequired";
  const peer = form.peer.trim();
  // peer 保持可空（§3.2 分享管理可先存后连）；非空按 base58 口径校验。
  // 连接类动作（测试连接/添加并开始对话）的目标必选由弹窗 requireTarget 前置拦截
  if (peer.length > 0 && !isValidPeerId(peer)) errors.peer = "peerInvalid";
  if (Array.from(form.alias.trim()).length > MAX_NICKNAME_CHARS) {
    errors.alias = "aliasTooLong";
  }
  const adminUrl = form.adminUrl?.trim() ?? "";
  if (adminUrl.length > 0 && !isValidAdminUrl(adminUrl)) {
    errors.adminUrl = "adminUrlInvalid";
  }
  return errors;
}

/** 分享导入端点稳定 id：按 peer 幂等，重复导入同一链接即刷新连接面（§8） */
export function shareEndpointId(peer: string): string {
  return "acp-share-" + peer;
}

/** 历史值下拉（§3.4 三律之三）：已保存 endpoint 的 wsUrl 去重、最近保存在前 */
export function wsUrlHistory(saved: Array<{ wsUrl: string }>): string[] {
  const seen = new Set<string>();
  const list: string[] = [];
  for (let i = saved.length - 1; i >= 0; i -= 1) {
    const url = saved[i]!.wsUrl;
    if (!url || seen.has(url)) continue;
    seen.add(url);
    list.push(url);
  }
  return list;
}

/** UX3 主字段目标下拉候选：console 发现面在册条目优先，既有收藏 peer 兜底
 *  （收藏语义不退化——历史连过的节点不因暂离发现面而消失）；按 peer 去重 */
export interface EndpointTargetOption {
  peer: string;
  label: string;
}

export function targetOptions(
  directory: Array<{ peer: string; name: string | null; source: string }>,
  saved: Array<{ peer: string; alias?: string }>,
): EndpointTargetOption[] {
  const out: EndpointTargetOption[] = [];
  const seen = new Set<string>();
  for (const entry of directory) {
    if (entry.source !== "discovered" || !entry.peer || seen.has(entry.peer)) continue;
    seen.add(entry.peer);
    out.push({ peer: entry.peer, label: entry.name ?? entry.peer });
  }
  for (const endpoint of saved) {
    if (!endpoint.peer || seen.has(endpoint.peer)) continue;
    seen.add(endpoint.peer);
    out.push({ peer: endpoint.peer, label: endpoint.alias ?? endpoint.peer });
  }
  return out;
}
