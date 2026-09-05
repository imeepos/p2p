import { useState } from "react";

// PeerId 前缀收敛常量：仪表盘/节点表/详情抽屉统一用这一个截断长度，
// 消除 12/10/16 三种并存的展示漂移。
export const PEER_ID_PREFIX_LEN = 12;

// PeerId 单元格：点击在截断与全文间切换（aria-expanded），全文等宽换行。
export function PeerIdCell({
  peerId,
  expandedTo = PEER_ID_PREFIX_LEN,
}: {
  peerId: string;
  expandedTo?: number;
}) {
  const [expanded, setExpanded] = useState(false);

  return (
    <button
      type="button"
      className="max-w-48 cursor-pointer truncate text-left font-mono text-xs hover:underline"
      title={peerId}
      aria-expanded={expanded}
      onClick={() => setExpanded((v) => !v)}
    >
      {expanded ? (
        <span className="break-all whitespace-normal">{peerId}</span>
      ) : (
        peerId.slice(0, expandedTo ?? 16)
      )}
    </button>
  );
}
