import { useChatStore } from "@/stores/chat-store";

// §3.2 rail 通讯录角标数据源：待处理（in 向）好友邀请数；out 向不占角标。
export function useIncomingInviteCount(): number {
  const invites = useChatStore((s) => s.invites);
  return (invites ?? []).filter((i) => i.direction === "in").length;
}
