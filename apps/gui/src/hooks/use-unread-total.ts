import { useAcpStore } from "@/acp/acp-store";
import { useA2aStore } from "@/a2a/a2a-store";
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";

// §2.3 rail 合计角标：四来源未读求和（1:1 + 群 + agent + a2a）。
function sumOf(record: Record<string, number>): number {
  let total = 0;
  for (const value of Object.values(record)) total += value;
  return total;
}

export function useUnreadTotal(): number {
  const unreadByPeer = useChatStore((s) => s.unreadByPeer);
  const unreadByGroup = useGroupStore((s) => s.unreadByGroup);
  const unreadByEndpoint = useAcpStore((s) => s.unreadByEndpoint);
  const unreadByAgent = useA2aStore((s) => s.unreadByAgent);
  return sumOf(unreadByPeer) + sumOf(unreadByGroup) + sumOf(unreadByEndpoint) + sumOf(unreadByAgent);
}
