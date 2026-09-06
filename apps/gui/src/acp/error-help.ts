import type { I18nKey } from "@/i18n/types";
import type { AcpCloseInfo } from "./protocol";

// F06：连接失败文案与内部码解耦——行内只出「原因 + 下一步动作」人话，
// 错误码/关闭码收进可复制详情。键集合与 ACP_ERROR_KEYS 的码一一对应。
export const ACP_ERROR_HELP: Record<string, I18nKey> = {
  endpointIncomplete: "uxgAcp.endpointIncomplete",
  initializeFailed: "uxgAcp.initializeFailed",
  sessionNewFailed: "uxgAcp.sessionNewFailed",
  sessionResumeFailed: "uxgAcp.sessionResumeFailed",
  sessionCloseFailed: "uxgAcp.sessionCloseFailed",
  promptFailed: "uxgAcp.promptFailed",
  setConfigFailed: "uxgAcp.setConfigFailed",
};

export function acpErrorHelpKey(code: string): I18nKey {
  return ACP_ERROR_HELP[code] ?? "uxgAcp.fallback";
}

// 关闭帧（WebSocket close）侧的人话：1006 空 reason 为 401 升级拒绝实测形态
const ACP_CLOSE_HELP: Record<string, I18nKey> = {
  denied: "acp.connection.closeDenied",
  "dial-failed": "acp.connection.closeDialFailed",
  abnormal: "acp.connection.closeAbnormal",
};

export function acpCloseHelpKey(info: AcpCloseInfo): I18nKey | null {
  if (info.kind === "abnormal" && info.code === 1006 && info.reason === "") {
    return "acp.reconnect.checkToken";
  }
  return ACP_CLOSE_HELP[info.kind] ?? null;
}

type Translate = (key: I18nKey) => string;

// 行内可读文案：lastError 优先（动作失败码），其次关闭帧，最后通用兜底
export function connectFailureText(
  t: Translate,
  lastError: string | null,
  closeInfo: AcpCloseInfo | null,
): string {
  if (lastError) return t(acpErrorHelpKey(lastError));
  if (closeInfo && closeInfo.kind !== "closed") {
    const key = acpCloseHelpKey(closeInfo);
    if (key) return t(key);
  }
  return t("uxgAcp.fallback");
}

// 可复制技术详情：错误码、关闭码、端点一次带全，供排查粘贴；无失败片段时空串
export function acpErrorDetail(parts: {
  lastError: string | null;
  closeInfo: AcpCloseInfo | null;
  wsUrl?: string;
}): string {
  const seg: string[] = [];
  if (parts.lastError) seg.push("error=" + parts.lastError);
  if (parts.closeInfo && parts.closeInfo.kind !== "closed") {
    seg.push(
      "close=" + parts.closeInfo.kind + "(code=" + parts.closeInfo.code + ")",
    );
  }
  if (parts.wsUrl) seg.push("ws=" + parts.wsUrl);
  return seg.join(" ");
}
