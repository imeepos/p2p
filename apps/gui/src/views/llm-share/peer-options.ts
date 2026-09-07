import { useMemo } from "react";

import { shortPeerId, type PickerOption } from "@/components/picker";
import { peerKnownName, usePeerNameSource } from "@/lib/peer-name";
import { selectPeerList, useNodeStore, type PeerEntry } from "@/stores/node-store";
import type { ChatFriendJson } from "@/lib/ipc-types";

// R2-05：PeerId 关联选择器候选 = 节点表可见对端（selectPeerList 与网络族
// 同一数据面），在册好友行显示昵称（peerKnownName 与 F02 同源），避免
// 用户手抄 44 位 PeerId。
export function peerPickerOptions(
  peers: PeerEntry[],
  friends: ChatFriendJson[],
): PickerOption[] {
  return peers.map((peer) => ({
    value: peer.peerId,
    label: peerKnownName(peer.peerId, friends) ?? shortPeerId(peer.peerId),
    hint: shortPeerId(peer.peerId),
  }));
}

export function usePeerPickerOptions(): PickerOption[] {
  const peers = useNodeStore(selectPeerList);
  const friends = usePeerNameSource();
  return useMemo(() => peerPickerOptions(peers, friends), [peers, friends]);
}
