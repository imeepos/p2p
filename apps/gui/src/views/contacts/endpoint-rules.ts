// agent endpoint 表单预校验（app-shell-redesign §3.4）：wsUrl 做 URL 格式
// 校验，错误码稳定经 i18n（contacts.endpoint.errors.<code>）；peer 沿用
// base58 32 字节口径；别名同昵称 trim ≤64 口径。
import { isValidPeerId, MAX_NICKNAME_CHARS } from "@/lib/chat-limits";

export type EndpointFieldError =
  | "wsUrlRequired"
  | "wsUrlInvalid"
  | "peerInvalid"
  | "aliasTooLong";

export interface EndpointFormErrors {
  wsUrl?: EndpointFieldError;
  peer?: EndpointFieldError;
  alias?: EndpointFieldError;
}

export function isValidWsUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return url.protocol === "ws:" || url.protocol === "wss:";
  } catch {
    return false;
  }
}

export function hasEndpointFormErrors(errors: EndpointFormErrors): boolean {
  return errors.wsUrl !== undefined || errors.peer !== undefined || errors.alias !== undefined;
}

export function validateEndpointForm(form: {
  wsUrl: string;
  peer: string;
  alias: string;
}): EndpointFormErrors {
  const errors: EndpointFormErrors = {};
  const wsUrl = form.wsUrl.trim();
  if (wsUrl.length === 0) errors.wsUrl = "wsUrlRequired";
  else if (!isValidWsUrl(wsUrl)) errors.wsUrl = "wsUrlInvalid";
  const peer = form.peer.trim();
  if (peer.length > 0 && !isValidPeerId(peer)) errors.peer = "peerInvalid";
  if (Array.from(form.alias.trim()).length > MAX_NICKNAME_CHARS) {
    errors.alias = "aliasTooLong";
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
