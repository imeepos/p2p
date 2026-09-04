// chat 页 descriptor：动作与界面入口（ChatFriendAddDialog / ChatFriendRemoveDialog /
// 聊天输入框）同源走 store/IPC，不经 DOM 模拟。removeFriend 是危险动作，
// registry 层强制 args.confirm === true（ACTION_CONFIRM_REQUIRED）。
import { markLocalWrite } from "@/lib/data-watch";
import { ipc } from "@/lib/ipc";
import { useChatStore } from "@/stores/chat-store";
import type { PageDescriptor, PageEntry } from "../page-registry";

const descriptor: PageDescriptor = {
  name: "chat",
  description: "IM 聊天页：好友会话文本发送与好友管理",
  actions: [
    {
      name: "sendText",
      description: "向好友发送文本（乐观更新，与聊天输入框同源）",
      args: [
        { name: "peer", type: "string", required: true, description: "好友 PeerId" },
        { name: "text", type: "string", required: true, description: "正文，trim 后 1..2000 字符" },
      ],
    },
    {
      name: "addFriend",
      description: "添加好友（PeerId 必填，昵称/地址选填），与添加好友表单同源",
      args: [
        { name: "peerId", type: "string", required: true, description: "好友 PeerId" },
        { name: "nickname", type: "string", required: false, description: "昵称，缺省空串" },
        { name: "addrs", type: "array", required: false, description: "多地址字符串列表" },
      ],
    },
    {
      name: "removeFriend",
      description: "移除好友（不删本地消息历史），与移除确认框同源",
      confirm: true,
      args: [
        { name: "peer", type: "string", required: true, description: "好友 PeerId" },
        { name: "confirm", type: "boolean", required: true, description: "危险动作，必须显式传 true" },
      ],
    },
  ],
};

async function execute(
  action: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  switch (action) {
    case "sendText":
      return useChatStore.getState().sendText(String(args.peer), String(args.text));
    case "addFriend": {
      const report = await ipc.chatFriendInvite(
        String(args.peerId).trim(),
        typeof args.nickname === "string" ? args.nickname.trim() : "",
        Array.isArray(args.addrs) ? args.addrs.map(String) : [],
      );
      markLocalWrite("chat");
      await Promise.all([
        useChatStore.getState().loadFriends(),
        useChatStore.getState().loadInvites(),
      ]);
      return report;
    }
    case "removeFriend": {
      const peer = String(args.peer);
      await ipc.chatFriendRemove(peer);
      markLocalWrite("chat");
      useChatStore.getState().forgetFriend(peer);
      return { removed: peer };
    }
    default:
      throw new Error(`chat 页未知动作: ${action}`);
  }
}

function state(): unknown {
  const snapshot = useChatStore.getState();
  return {
    selectedPeer: snapshot.selectedPeer,
    friends: snapshot.friends.map((friend) => ({
      peerId: friend.peerId,
      nickname: friend.nickname,
    })),
  };
}

export const chatPage: PageEntry = { descriptor, execute, state };
