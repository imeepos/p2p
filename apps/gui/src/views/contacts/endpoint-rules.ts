// agent endpoint 表单预校验（app-shell-redesign §3.4）：wsUrl 做 URL 格式
// 校验，错误码稳定经 i18n（contacts.endpoint.errors.<code>）；peer 沿用
// base58 32 字节口径；别名同昵称 trim ≤64 口径；token 必填（console 随机
// 发布，粘贴一次后随草稿复用）；adminUrl 可选但填了必须合法（分享管理面）。
import { isValidPeerId, MAX_NICKNAME_CHARS } from "@/lib/chat-limits";

export type EndpointFieldError =
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

export function validateEndpointForm(form: {
  wsUrl: string;
  token: string;
  peer: string;
  alias: string;
  adminUrl?: string;
}): EndpointFormErrors {
  const errors: EndpointFormErrors = {};
  const wsUrl = form.wsUrl.trim();
  if (wsUrl.length === 0) errors.wsUrl = "wsUrlRequired";
  else if (!isValidWsUrl(wsUrl)) errors.wsUrl = "wsUrlInvalid";
  if (form.token.trim().length === 0) errors.token = "tokenRequired";
  const peer = form.peer.trim();
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
