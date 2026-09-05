import { render } from "@testing-library/react";
import type { Mock } from "vitest";
import { MemoryRouter } from "react-router-dom";

import type {
  ChatFriendJson,
  ChatMessageJson,
  GroupJson,
  GroupMessageJson,
} from "@/lib/ipc-types";
import { useAcpStore } from "@/acp/acp-store";
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";
import { NARROW_CHAT_QUERY } from "@/hooks/use-media-query";
import { ChatPage } from "@/views/chat/chat-page";

// ChatPage 验收测试共用夹具：三来源播种（mock 与 store 同源，防 load 覆盖）、
// 路由挂载与 jsdom 视口模拟（§2.1 <768 单栏互斥）。

export const PEER = "3xY9whporr5wt4u8t1z33G85F6K5CBEETKGSHWGPNRRe";
export const PEER_B = "3xY9whporr5wt4u8t1z33G85F6K5CBEETKGSHWGPNRRf";
export const GROUP_ID = "11111111-2222-3333-4444-555555555555";
export const ENDPOINT_ID = "ep-page-1";

/** 各测试文件 vi.hoisted 的 ipc mock 面（vi.mock 须逐文件注册，无法共享） */
export interface MediaMocks {
  friends: Mock<() => Promise<ChatFriendJson[]>>;
  history: Mock<(peer: string) => Promise<ChatMessageJson[]>>;
  groupList: Mock<() => Promise<GroupJson[]>>;
  invites: Mock<() => Promise<never[]>>;
  nodeStatus: Mock<() => Promise<{ peerId: string; running: boolean }>>;
}

export function friendOf(id: string, nickname: string): ChatFriendJson {
  return { peerId: id, nickname, addrs: [], note: "备注甲" };
}

export function msg(
  id: string,
  peer: string,
  sender: "me" | "them",
  text: string,
  tsMs: number,
): ChatMessageJson {
  return { id, peer, sender, kind: "text", tsMs, text, media: null, status: "delivered" };
}

export function groupFixture(): GroupJson {
  return {
    groupId: GROUP_ID,
    name: "项目组",
    owner: PEER,
    members: [PEER, "member-b"],
    rev: 1,
    state: "active",
    tsMs: 500,
  };
}

export function groupMsg(id: string, senderId: string, text: string, tsMs: number): GroupMessageJson {
  return {
    id, groupId: GROUP_ID, senderId, kind: "text", tsMs,
    text, media: null, status: "delivered", acks: [],
  };
}

export function agentEndpoint() {
  return {
    wsUrl: "ws://127.0.0.1:8787",
    token: "t",
    peer: "agent-peer",
    endpointId: ENDPOINT_ID,
    alias: "助手甲",
  };
}

/** 三来源播种：mock 与 store 播种同源——ChatPage 挂载即拉取，防 load 后覆盖 */
export function seedAll(mocks: MediaMocks): void {
  mocks.friends.mockResolvedValue([friendOf(PEER, "小圆"), friendOf(PEER_B, "阿北")]);
  mocks.history.mockImplementation(async (peer: string) => {
    if (peer === PEER) return [msg("pf1", PEER, "them", "晚安", 3_000)];
    if (peer === PEER_B) return [msg("pf2", PEER_B, "me", "早", 2_000)];
    return [];
  });
  mocks.groupList.mockResolvedValue([groupFixture()]);
  mocks.invites.mockResolvedValue([]);
  mocks.nodeStatus.mockResolvedValue({ peerId: PEER, running: true });
  useChatStore.setState({
    friends: [friendOf(PEER, "小圆"), friendOf(PEER_B, "阿北")],
    friendsLoaded: true,
    friendsError: null,
    selectedPeer: null,
    messagesByPeer: {},
    lastMessageByPeer: {
      [PEER]: msg("pf1", PEER, "them", "晚安", 3_000),
      [PEER_B]: msg("pf2", PEER_B, "me", "早", 2_000),
    },
    unreadByPeer: {},
    historyLoading: {},
    historyLoaded: {},
    hasMore: {},
    historyError: {},
    olderError: {},
  });
  useGroupStore.setState({
    groups: [groupFixture()],
    groupsLoaded: true,
    groupsError: null,
    friends: [friendOf(PEER, "小圆"), friendOf("member-b", "阿北")],
    friendsLoaded: true,
    selfPeerId: PEER,
    selectedGroupId: null,
    messagesByGroup: {},
    lastMessageByGroup: { [GROUP_ID]: groupMsg("gm1", "member-b", "开会啦", 4_000) },
    unreadByGroup: {},
    historyLoading: {},
    historyLoaded: {},
    hasMore: {},
    historyError: {},
    olderError: {},
  });
  useAcpStore.setState({
    saved: [agentEndpoint()],
    phase: "idle",
    activePeer: null,
    activeEndpointId: null,
    focusedEndpointId: null,
    unreadByEndpoint: {},
    lastInteractionByEndpoint: {},
  });
}

export function renderAt(entry = "/chat"): void {
  render(
    <MemoryRouter initialEntries={[entry]}>
      <ChatPage />
    </MemoryRouter>,
  );
}

// ---- jsdom 视口模拟（<768 断点钩子走 matchMedia，测试可驱动）----
let narrowMatches = false;
const mediaListeners = new Set<(e: { matches: boolean }) => void>();

export function installMatchMedia(): void {
  window.matchMedia = ((query: string) => ({
    get matches() {
      return query === NARROW_CHAT_QUERY ? narrowMatches : false;
    },
    media: query,
    onchange: null,
    addEventListener: (_: string, cb: (e: { matches: boolean }) => void) => {
      mediaListeners.add(cb);
    },
    removeEventListener: (_: string, cb: (e: { matches: boolean }) => void) => {
      mediaListeners.delete(cb);
    },
    addListener: (cb: (e: { matches: boolean }) => void) => {
      mediaListeners.add(cb);
    },
    removeListener: (cb: (e: { matches: boolean }) => void) => {
      mediaListeners.delete(cb);
    },
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
}

export function setViewportNarrow(next: boolean): void {
  narrowMatches = next;
  for (const cb of [...mediaListeners]) cb({ matches: next });
}

export function resetViewport(): void {
  narrowMatches = false;
  mediaListeners.clear();
}
