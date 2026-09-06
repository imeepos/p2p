import { useCallback, useEffect } from "react";

import type { ChatFriendJson } from "@/lib/ipc-types";
import { useChatStore } from "@/stores/chat-store";

// F02/F21 统一缩略口径：前 6 后 4（审计两处同源，收成一个常量）。
const SHORT_HEAD = 6;
const SHORT_TAIL = 4;

export function shortPeerId(peerId: string): string {
  if (peerId.length <= SHORT_HEAD + SHORT_TAIL) return peerId;
  return peerId.slice(0, SHORT_HEAD) + "…" + peerId.slice(-SHORT_TAIL);
}

// F02 口径：好友昵称优先、备注次之；非好友返回 null，由调用方回退缩略 ID。
// 与 friend-row/conversation-entry 的「昵称 || 缩略」惯例同源，不在展示层各写一套。
export function peerKnownName(
  peerId: string,
  friends: ChatFriendJson[],
): string | null {
  const friend = friends.find((f) => f.peerId === peerId);
  if (!friend) return null;
  return friend.nickname || friend.note || null;
}

// 事件流等单行场景：好友显示「名字 (缩略ID)」，否则缩略 ID。
export function peerDisplayLabel(
  peerId: string,
  friends: ChatFriendJson[],
): string {
  const name = peerKnownName(peerId, friends);
  return name ? name + " (" + shortPeerId(peerId) + ")" : shortPeerId(peerId);
}

let nameSourceWarmed = false;

// 网络族页面没有好友簿装载入口：首个名称单元格挂载时预热一次读模型；
// 拉取失败由 chat-store 记 friendsError 并告警，这里放开标记允许下次重试。
export function usePeerNameSource(): ChatFriendJson[] {
  const friends = useChatStore((s) => s.friends);
  const loaded = useChatStore((s) => s.friendsLoaded);
  useEffect(() => {
    if (nameSourceWarmed || loaded) return;
    nameSourceWarmed = true;
    void useChatStore.getState().loadFriends();
  }, [loaded]);
  return friends;
}

export function usePeerKnownName(peerId: string): string | null {
  const friends = usePeerNameSource();
  return peerKnownName(peerId, friends);
}

export function usePeerNameLabel(): (peerId: string) => string {
  const friends = usePeerNameSource();
  return useCallback(
    (peerId: string) => peerDisplayLabel(peerId, friends),
    [friends],
  );
}
