export interface DialTarget {
  peerId: string;
  addr: string;
}

// 32 字节 PeerId 的 base58 典型长度 43-45；字节级权威校验在桥接层（proto.rs）。
const PEER_ID_RE = /^[1-9A-HJ-NP-Za-km-z]{43,45}$/;
const IPV4_RE = /^(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}$/;
const IPV6ISH_RE = /^[0-9a-fA-F:]+$/;

function validHost(host: string): boolean {
  return IPV4_RE.test(host) || (host.includes(":") && IPV6ISH_RE.test(host));
}

// 契约 §6：<peer_id>@<addr>；addr 为 ip/u端口（QUIC）或 ip/t端口（TCP）。
// 视图预检与 mock 共用；真实调用的最终裁决在 src-tauri proto::parse_target。
export function parseDialTarget(raw: string): DialTarget | null {
  const at = raw.indexOf("@");
  if (at <= 0 || at === raw.length - 1) return null;
  const peerId = raw.slice(0, at);
  const addr = raw.slice(at + 1);
  if (!PEER_ID_RE.test(peerId)) return null;
  const slash = addr.lastIndexOf("/");
  if (slash <= 0) return null;
  const host = addr.slice(0, slash);
  const tail = addr.slice(slash + 1);
  const kind = tail.charAt(0);
  const port = Number(tail.slice(1));
  if (!validHost(host)) return null;
  if ((kind !== "u" && kind !== "t") || !Number.isInteger(port)) return null;
  if (port < 1 || port > 65535) return null;
  return { peerId, addr };
}
