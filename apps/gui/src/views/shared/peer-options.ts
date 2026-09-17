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

// 好友源候选（与 generic-tunnel-card 原 friendOptions 同构收编）：
// 离线好友也可选——「拨号/授权不在场好友」恰是手抄 PeerId 的高峰场景。
export function friendPickerOptions(
  friends: ChatFriendJson[],
): PickerOption[] {
  return friends.map((friend) => ({
    value: friend.peerId,
    label: friend.nickname || friend.note || shortPeerId(friend.peerId),
    hint: shortPeerId(friend.peerId),
  }));
}

export function useFriendPickerOptions(): PickerOption[] {
  const friends = usePeerNameSource();
  return useMemo(() => friendPickerOptions(friends), [friends]);
}
