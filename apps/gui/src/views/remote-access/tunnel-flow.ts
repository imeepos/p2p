// tunnel 分享流纯前端派生工具：ws 地址派生、错误码识别、对端 CLI 命令拼装。
// 数据面仅消费既有契约字段（TunnelOpenReport/TunnelStatusReport/serve.allow），
// 不新增 IPC。

// tunnel.md §4 错误码闭集（六值，MUST NOT 使用闭集外的 code）。
export const TUNNEL_ERROR_CODES = [
  "bad_ticket",
  "target_not_allowed",
  "busy",
  "dial_failed",
  "io",
  "shutdown",
] as const;

export type TunnelErrorCode = (typeof TUNNEL_ERROR_CODES)[number];

export function isTunnelErrorCode(value: string): value is TunnelErrorCode {
  return (TUNNEL_ERROR_CODES as readonly string[]).includes(value);
}

// 访侧成功态派生 ws:// 地址：优先 localAddr（127.0.0.1:<port> 字面量），
// 回退从 http:// 入口链接换 scheme；两者皆缺返回 null（不做无依据拼装）。
export function deriveWsAddr(
  openUrl: string | null,
  localAddr: string | null,
): string | null {
  if (localAddr) return "ws://" + localAddr;
  if (openUrl && openUrl.startsWith("http://")) {
    return "ws://" + openUrl.slice("http://".length);
  }
  return null;
}

// 从错误文案中识别六值闭集错误码（词边界精确匹配），无码返回 null。
export function matchErrorCode(message: string): TunnelErrorCode | null {
  for (const code of TUNNEL_ERROR_CODES) {
    if (new RegExp("\\b" + code + "\\b").test(message)) return code;
  }
  return null;
}

// serve.allow 条目为 127.0.0.1:<port> 字面量，表格端口列只取端口段。
export function targetPort(target: string): string {
  const prefix = "127.0.0.1:";
  return target.startsWith(prefix) ? target.slice(prefix.length) : target;
}

// 对端 CLI 原文（apps/cli connect.rs：--peer / --target 皆必填）。
export function cliConnectCommand(peer: string, target: string): string {
  return "p2pctl tunnel connect --peer " + peer + " --target " + target;
}
