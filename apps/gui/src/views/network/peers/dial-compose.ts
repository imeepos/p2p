// 结构化拨号三段（PeerId/地址/端口 + 传输）与契约 §6 复合目标的换算：
// <peer_id>@<ip>/<u|t><端口>。语法裁决仍归 parseDialTarget（lib/dial-target），
// 这里只做拆装与端口即时校验口径（F14）。
import { parseDialTarget } from "@/lib/dial-target";

export type DialTransport = "u" | "t";

export interface DialSegments {
  peerId: string;
  addr: string;
  port: string;
  transport: DialTransport;
}

// 地址簿条目地址（<ip>/<u|t><端口>）→ 主机 + 传输 + 端口；不可拆整条不带入。
export function splitDialAddr(addr: string): { host: string; transport: DialTransport; port: string } | null {
  const slash = addr.lastIndexOf("/");
  if (slash <= 0) return null;
  const host = addr.slice(0, slash);
  const tail = addr.slice(slash + 1);
  const kind = tail.charAt(0);
  const port = tail.slice(1);
  if ((kind !== "u" && kind !== "t") || !/^\d{1,5}$/.test(port)) return null;
  return { host, transport: kind, port };
}

export function isValidDialPort(value: string): boolean {
  if (!/^\d{1,5}$/.test(value)) return false;
  const port = Number(value);
  return Number.isInteger(port) && port >= 1 && port <= 65535;
}

// 三段 → 复合目标；任一段为空返回 null（未填完不是语法错误，不提示红字）。
export function composeDialTarget(segments: DialSegments): string | null {
  const peerId = segments.peerId.trim();
  const addr = segments.addr.trim();
  const port = segments.port.trim();
  if (peerId.length === 0 || addr.length === 0 || port.length === 0) return null;
  return `${peerId}@${addr}/${segments.transport}${port}`;
}

// 复合目标 → 三段（URL 契约 ?dial=<目标> 预填）；不可解析时原串落到 PeerId 段。
export function splitDialTarget(raw: string): DialSegments {
  const parsed = parseDialTarget(raw);
  if (parsed) {
    const split = splitDialAddr(parsed.addr);
    if (split) {
      return { peerId: parsed.peerId, addr: split.host, port: split.port, transport: split.transport };
    }
    return { peerId: parsed.peerId, addr: parsed.addr, port: "", transport: "u" };
  }
  return { peerId: raw, addr: "", port: "", transport: "u" };
}
