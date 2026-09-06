import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson, FriendInviteJson } from "@/lib/ipc-types";
import { selectPeerList, useNodeStore } from "@/stores/node-store";
import { useChatStore } from "@/stores/chat-store";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(async () => []),
    invites: vi.fn<() => Promise<FriendInviteJson[]>>(async () => []),
    addFriend: vi.fn(),
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatInvitesList: mocks.invites,
    chatFriendInvite: mocks.addFriend,
    onNodeEvent: () => Promise.resolve(() => {}),
  },
}));

import "@/i18n";
import { ChatFriendAddDialog, friendPickOptions } from "./chat-friend-add-dialog";

// 真实 base58（解码恰 32 字节），与后端 parse_peer_id 同口径的合法夹具
const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";
const PEER2 = "2cLxSV5Nr7oBdzPhCaLD5XMuwkRDWsyf9m3DpZCTHs2a";

function seedDiscovered(peerIds: string[]): void {
  const peers = Object.fromEntries(
    peerIds.map((peerId) => [
      peerId,
      {
        peerId,
        addrs: ["192.168.1.9/u34001"],
        source: "mdns",
        connected: false,
        lastSeenMs: 1,
        hops: [],
      },
    ]),
  );
  useNodeStore.setState({ peers });
}

beforeEach(() => {
  mocks.friends.mockReset().mockResolvedValue([]);
  mocks.invites.mockReset().mockResolvedValue([]);
  mocks.addFriend.mockReset();
  useChatStore.setState({ friends: [], invites: [], friendsLoaded: true, friendsError: null });
  useNodeStore.setState({ peers: {} });
});

describe("添加好友弹窗：发现清单选择器（F03）", () => {
  it("从发现清单选择节点自动填入 PeerId，自由文本输入保留", async () => {
    seedDiscovered([PEER]);
    render(<ChatFriendAddDialog open onOpenChange={() => {}} />);
    fireEvent.click(screen.getByTestId("friend-add-picker"));
    fireEvent.click(await screen.findByRole("option"));
    const input = screen.getByLabelText("PeerId") as HTMLInputElement;
    await waitFor(() => expect(input.value).toBe(PEER));
    // 自由文本兜底：选择后仍可手工改写
    fireEvent.change(input, { target: { value: PEER2 } });
    expect(input.value).toBe(PEER2);
  });

  it("候选名以好友昵称优先，无昵称用缩略 PeerId", () => {
    useChatStore.setState({
      friends: [{ peerId: PEER, nickname: "小圆", addrs: [], note: null }],
    });
    expect(friendPickOptions([{ peerId: PEER }, { peerId: PEER2 }], useChatStore.getState().friends)).toEqual([
      { value: PEER, label: "小圆", hint: expect.any(String) },
      { value: PEER2, label: PEER2.slice(0, 12) + "…" + PEER2.slice(-8), hint: expect.any(String) },
    ]);
  });
});

describe("添加好友弹窗：行内校验读屏可达（F24）", () => {
  it("PeerId 校验失败：输入 aria-invalid 且 describedby 指向 role=alert 错误节点", () => {
    render(<ChatFriendAddDialog open onOpenChange={() => {}} />);
    fireEvent.change(screen.getByLabelText("PeerId"), { target: { value: "!!!bad!!!" } });
    fireEvent.click(screen.getByTestId("friend-add-submit"));
    const input = screen.getByLabelText("PeerId");
    expect(input.getAttribute("aria-invalid")).toBe("true");
    const describedBy = input.getAttribute("aria-describedby");
    expect(describedBy).toBe("friend-add-peer-id-error");
    const errorNode = document.getElementById(describedBy!);
    expect(errorNode?.getAttribute("role")).toBe("alert");
    expect(errorNode?.textContent).toContain("PeerId 非法");
    expect(mocks.addFriend).not.toHaveBeenCalled();
  });
});

describe("跨卡 URL 契约：#/contacts?add=<peerId> 预填", () => {
  it("挂载即以 initialPeerId 预填 PeerId 输入", () => {
    render(<ChatFriendAddDialog open onOpenChange={() => {}} initialPeerId={PEER} />);
    expect((screen.getByLabelText("PeerId") as HTMLInputElement).value).toBe(PEER);
  });

  it("节点表在册节点为空时选择器进空态，不阻塞自由文本", () => {
    render(<ChatFriendAddDialog open onOpenChange={() => {}} />);
    expect(selectPeerList(useNodeStore.getState())).toEqual([]);
    expect(screen.getByLabelText("PeerId")).toBeTruthy();
  });
});
