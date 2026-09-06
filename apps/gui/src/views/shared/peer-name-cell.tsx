import { cn } from "@/lib/utils";
import { shortPeerId, usePeerKnownName } from "@/lib/peer-name";

// F02 网络族表格首列：已知好友行主行显昵称/备注，PeerId 缩略（前 6 后 4）
// 为副行；非好友仅缩略 ID。title 悬挂完整 ID，复制入口由行内 CopyButton 承担。
export function PeerNameCell({ peerId }: { peerId: string }) {
  const name = usePeerKnownName(peerId);
  return (
    <div className="min-w-0" title={peerId}>
      {name ? (
        <div className="truncate text-sm font-medium">{name}</div>
      ) : null}
      <div
        className={cn("truncate font-mono text-xs", name && "text-muted-foreground")}
      >
        {shortPeerId(peerId)}
      </div>
    </div>
  );
}
