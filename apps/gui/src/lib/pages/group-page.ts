// group 页 descriptor：群聊页的群组语义登记面。actions 与 group-store
// 公开方法同源（真实 store/IPC 调用，无 DOM 模拟）；store 动作失败直接
// 抛错，由 page-registry 统一转 ACTION_FAILED 结构化返回。
import { ipc } from "@/lib/ipc";
import { useGroupStore } from "@/stores/group-store";
import type { PageDescriptor, PageEntry } from "../page-registry";

const descriptor: PageDescriptor = {
  name: "group",
  description: "群聊页：群组生命周期与会话驱动",
  actions: [
    {
      name: "refresh",
      description: "刷新群组清单（只读）",
      args: [],
    },
    {
      name: "create",
      description: "创建群聊并加入清单（与新建群聊对话框同源）",
      args: [
        { name: "name", type: "string", required: true, description: "群名" },
        { name: "memberIds", type: "array", required: true, description: "初始成员 PeerId 列表" },
      ],
    },
    {
      name: "select",
      description: "选中群组并加载其最近历史",
      args: [
        { name: "groupId", type: "string", required: true, description: "目标群 id" },
      ],
    },
    {
      name: "sendText",
      description: "向指定群发送文本消息",
      args: [
        { name: "groupId", type: "string", required: true, description: "目标群 id" },
        { name: "text", type: "string", required: true, description: "消息文本" },
      ],
    },
    {
      name: "invite",
      description: "邀请成员加入群聊",
      args: [
        { name: "groupId", type: "string", required: true, description: "目标群 id" },
        { name: "memberIds", type: "array", required: true, description: "被邀请人 PeerId 列表" },
      ],
    },
    {
      name: "rename",
      description: "重命名群聊",
      args: [
        { name: "groupId", type: "string", required: true, description: "目标群 id" },
        { name: "name", type: "string", required: true, description: "新群名" },
      ],
    },
    {
      name: "kick",
      description: "将成员移出群聊（危险动作）",
      confirm: true,
      args: [
        { name: "groupId", type: "string", required: true, description: "目标群 id" },
        { name: "memberId", type: "string", required: true, description: "被移出成员 PeerId" },
        { name: "confirm", type: "boolean", required: true, description: "危险动作，必须显式传 true" },
      ],
    },
    {
      name: "leave",
      description: "退出群聊（危险动作）",
      confirm: true,
      args: [
        { name: "groupId", type: "string", required: true, description: "目标群 id" },
        { name: "confirm", type: "boolean", required: true, description: "危险动作，必须显式传 true" },
      ],
    },
    {
      name: "disband",
      description: "解散群聊（危险动作）",
      confirm: true,
      args: [
        { name: "groupId", type: "string", required: true, description: "目标群 id" },
        { name: "confirm", type: "boolean", required: true, description: "危险动作，必须显式传 true" },
      ],
    },
  ],
};

function groupSnapshot(): unknown {
  const s = useGroupStore.getState();
  return { groups: s.groups, selectedGroupId: s.selectedGroupId, groupsError: s.groupsError };
}

async function execute(
  action: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  const store = useGroupStore.getState();
  switch (action) {
    case "refresh":
      await store.loadGroups();
      break;
    case "create": {
      const group = await ipc.groupCreate(
        String(args.name),
        (args.memberIds as string[]).map(String),
      );
      store.upsertGroup(group);
      break;
    }
    case "select":
      await store.selectGroup(String(args.groupId));
      break;
    case "sendText":
      await store.sendText(String(args.groupId), String(args.text));
      break;
    case "invite":
      await store.invite(String(args.groupId), (args.memberIds as string[]).map(String));
      break;
    case "rename":
      await store.rename(String(args.groupId), String(args.name));
      break;
    case "kick":
      await store.kick(String(args.groupId), String(args.memberId));
      break;
    case "leave":
      await store.leave(String(args.groupId));
      break;
    case "disband":
      await store.disband(String(args.groupId));
      break;
    default:
      throw new Error(`group 页未知动作: ${action}`);
  }
  return groupSnapshot();
}

export const groupPage: PageEntry = { descriptor, execute, state: groupSnapshot };
