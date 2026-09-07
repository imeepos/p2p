import { shortPeerId } from "@/lib/peer-name";

// R2-17：全站 PeerId 缩略唯一口径——前 6…后 4（复用 F02 shortPeerId，
// 不另造常量），悬停 title 展示完整 ID。需要显式复制按钮的场景与
// CopyButton 并排（节点身份卡先例）；整行点击复制的场景（命令面板
// 节点项）由行自身动作承担，本组件只负责缩略展示。
export function PeerIdShort({
  peerId,
  className,
}: {
  peerId: string;
  className?: string;
}) {
  return (
    <span className={className} title={peerId}>
      {shortPeerId(peerId)}
    </span>
  );
}
