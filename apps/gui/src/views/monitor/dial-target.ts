const PEER_ID_RE = /^[1-9A-HJ-NP-Za-km-z]{8,}$/;
const ADDR_RE = /^([^@\s/]+)\/([ut])([0-9]{1,5})$/;

export interface DialTarget {
  peerId: string;
  addr: string;
}

// 契约 §6：<peer_id>@<addr>；addr 为 ip/u端口（QUIC）或 ip/t端口（TCP）。
export function parseDialTarget(raw: string): DialTarget | null {
  const at = raw.indexOf("@");
  if (at <= 0 || at === raw.length - 1) return null;
  const peerId = raw.slice(0, at);
  const addr = raw.slice(at + 1);
  if (!PEER_ID_RE.test(peerId)) return null;
  const matched = ADDR_RE.exec(addr);
  if (!matched) return null;
  const port = Number(matched[3]);
  if (port < 1 || port > 65535) return null;
  return { peerId, addr };
}
