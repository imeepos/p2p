import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson, FriendInviteJson, InviteReportJson, NodeEventHandler } from "@/lib/ipc-types";

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    addFriend: vi.fn<
      (peerId: string, nickname: string, addrs: string[]) => Promise<InviteReportJson>
    >(),
    invites: vi.fn<() => Promise<FriendInviteJson[]>>(async () => []),
    accept: vi.fn<(peerId: string, nickname: string) => Promise<unknown>>(),
    reject: vi.fn<(peerId: string) => Promise<void>>(),
    cancel: vi.fn<(peerId: string) => Promise<boolean>>(),
    history: vi.fn<
      (peer: string, beforeId?: string | null, limit?: number) => Promise<unknown[]>
    >(),
    send: vi.fn(),
    eventHandler: { current: null as NodeEventHandler | null },
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatFriendInvite: mocks.addFriend,
    chatInvitesList: mocks.invites,
    chatInviteAccept: mocks.accept,
    chatInviteReject: mocks.reject,
    chatInviteCancel: mocks.cancel,
    chatHistory: mocks.history,
    chatSend: mocks.send,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.eventHandler.current = handler;
      return Promise.resolve(() => {});
    },
  },
}));

import "@/i18n";
import { useChatStore } from "@/stores/chat-store";
import { ChatFriendAddDialog } from "@/components/chat/chat-friend-add-dialog";
import { ChatInvitePanel } from "@/components/chat/chat-invite-panel";

// 真实 base58（解码恰 32 字节），与后端 parse_peer_id 同口径的合法夹具
const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";

function friendOf(peerId: string, nickname: string): ChatFriendJson {
  return { peerId, nickname, addrs: [], note: null };
}

beforeEach(() => {
  mocks.friends.mockReset().mockResolvedValue([]);
  mocks.addFriend.mockReset();
  mocks.history.mockReset().mockResolvedValue([]);
  mocks.send.mockReset();
  useChatStore.setState({
    invites: [],
    friends: [],
    friendsLoaded: false,
    friendsError: null,
    selectedPeer: null,
    messagesByPeer: {},
    lastMessageByPeer: {},
    historyLoading: {},
    historyLoaded: {},
    hasMore: {},
  });
});

// P1 起 /chat 列表不再承载好友管理入口（归 P2 通讯录），本文件直挂对话框
// 组件覆盖表单校验/错误路径/旅程；入口可达性回归随 P2 迁移重建。
async function openAddDialog(): Promise<void> {
  render(
    <>
      <ChatInvitePanel />
      <ChatFriendAddDialog open onOpenChange={() => {}} />
    </>,
  );
  await waitFor(() => expect(screen.getByTestId("friend-add-dialog")).toBeTruthy());
}

describe("ChatView 添加好友校验与错误路径", () => {
  it("非法 PeerId：前端预校验拦截并红字提示，不触达后端", async () => {
    await openAddDialog();
    fireEvent.change(screen.getByLabelText("PeerId"), {
      target: { value: "!!!not-base58!!!" },
    });
    fireEvent.click(screen.getByTestId("friend-add-submit"));
    await waitFor(() =>
      expect(
        screen.getByText("PeerId 非法：需为合法 base58 且解码后为 32 字节"),
      ).toBeTruthy(),
    );
    expect(mocks.addFriend).not.toHaveBeenCalled();
  });

  it("重复添加：后端拒绝原文展示表单内，已填内容保留，列表不出现重复条目", async () => {
    mocks.friends.mockResolvedValue([friendOf(PEER, "小圆")]);
    mocks.addFriend.mockRejectedValue(new Error(`该节点已是好友：${PEER}`));
    const logSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    // 好友簿就位（原由 ChatView 挂载时 loadFriends 填充）
    useChatStore.setState({ friends: [friendOf(PEER, "小圆")], friendsLoaded: true });
    await openAddDialog();
    fireEvent.change(screen.getByLabelText("PeerId"), { target: { value: PEER } });
    fireEvent.click(screen.getByTestId("friend-add-submit"));
    await waitFor(() =>
      expect(screen.getByTestId("friend-add-error").textContent).toContain(
        `该节点已是好友：${PEER}`,
      ),
    );
    expect((screen.getByLabelText("PeerId") as HTMLInputElement).value).toBe(PEER);
    expect(logSpy).toHaveBeenCalledWith("[chat] 添加好友失败", expect.any(Error));
    // 列表不出现重复条目：store 权威状态恰一条
    expect(useChatStore.getState().friends.filter((f) => f.nickname === "小圆")).toHaveLength(1);
    logSpy.mockRestore();
  });

  it("后端拒绝（自加为好友）：不白屏、表单保留已填内容、失败留日志", async () => {
    mocks.addFriend.mockRejectedValue(new Error(`不能把自己加为好友：${PEER}`));
    const logSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    await openAddDialog();
    fireEvent.change(screen.getByLabelText("PeerId"), { target: { value: PEER } });
    fireEvent.change(screen.getByLabelText("昵称（可选）"), {
      target: { value: "自己" },
    });
    fireEvent.click(screen.getByTestId("friend-add-submit"));
    await waitFor(() =>
      expect(screen.getByTestId("friend-add-error").textContent).toContain(
        "不能把自己加为好友",
      ),
    );
    expect((screen.getByLabelText("昵称（可选）") as HTMLInputElement).value).toBe("自己");
    expect(screen.getByTestId("friend-add-dialog")).toBeTruthy();
    expect(logSpy).toHaveBeenCalled();
    logSpy.mockRestore();
  });
});

describe("ChatView 从零开始旅程", () => {
  it("从零开始：零好友空态引导直达表单，提交后邀请挂起（同意前不建好友），发文本上屏", async () => {
    mocks.friends.mockResolvedValue([]); // 邀请制：同意前好友簿恒空
    mocks.addFriend.mockResolvedValue({
      invite: {
        peerId: PEER,
        nickname: "小圆",
        addrs: [],
        note: null,
        direction: "out",
        tsMs: Date.now(),
        delivered: true,
      },
      delivered: true,
    });
    mocks.send.mockResolvedValue({
      message: {
        id: "m1",
        peer: PEER,
        sender: "me",
        kind: "text",
        tsMs: Date.now(),
        text: "你好啊",
        media: null,
        status: "delivered",
      },
      delivered: true,
    });

    await openAddDialog();
    fireEvent.change(screen.getByLabelText("PeerId"), { target: { value: PEER } });
    fireEvent.change(screen.getByLabelText("昵称（可选）"), {
      target: { value: "小圆" },
    });
    fireEvent.click(screen.getByTestId("friend-add-submit"));
    expect(mocks.addFriend).toHaveBeenCalledWith(PEER, "小圆", []);
    mocks.invites.mockResolvedValue([
      {
        peerId: PEER,
        nickname: "小圆",
        addrs: [],
        note: null,
        direction: "out",
        tsMs: Date.now(),
        delivered: true,
      },
    ]);
    // 同意前好友簿空：挂起提示出现（聊天输入条不自动打开）
    await waitFor(() =>
      expect(screen.getByTestId("chat-invite-out-" + PEER)).toBeTruthy(),
    );
  });
});

describe("IPC 调用点静态守卫", () => {
  const SRC = join(process.cwd(), "src");
  const SCAN_DIRS = ["views", "components"];
  // 显式豁免清单：确无 views/components 入口的方法必须登记原因（防后端有能力、界面无入口复发）
  const EXEMPT: Record<string, string> = {
    chatFriendsList: "好友列表刷新统一由 stores/chat-store.loadFriends 调用（数据层）",
    chatInvitesList: "邀请列表刷新统一由 stores/chat-store.loadInvites 调用（数据层）",
    chatInviteAccept: "同意邀请统一由 stores/chat-store.acceptInvite 调用（数据层）",
    chatInviteReject: "拒绝邀请统一由 stores/chat-store.rejectInvite 调用（数据层）",
    chatInviteCancel: "撤回邀请统一由 stores/chat-store.cancelInvite 调用（数据层）",
    chatHistory: "历史加载统一由 stores/chat-store（selectPeer/loadOlder/loadFriends）调用",
    chatSend: "消息发送统一由 stores/chat-store.sendText/sendMedia 调用（Composer 经 store）",
    chatMediaFile: "媒体展示当前直接消费消息内 path，无独立入口；接媒体落盘地址时补调用点",
  };

  function listFiles(dir: string): string[] {
    // withFileTypes 免去逐项 statSync：全量套件并行下 fs 系统调用排队是超时主源
    return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) return listFiles(path);
      return /.(tsx|ts)$/.test(path) && !/\.test\.(tsx|ts)$/.test(path) ? [path] : [];
    });
  }

  function chatMethodsOfIpcBackend(): string[] {
    const source = readFileSync(join(SRC, "lib", "ipc-types.ts"), "utf8");
    const start = source.indexOf("export interface IpcBackend");
    const end = source.indexOf("export interface DiagBackend");
    return [...source.slice(start, end).matchAll(/^\s{2}(chat\w+)\(/gm)].map((m) => m[1]!);
  }

  it(
    "IpcBackend 全部 chat 方法在 views/components 有非测试调用点（豁免清单制）",
    // vitest 4：options 位于第二参；纯 fs 扫描用例在全量套件并行下 IO 排队
    // 远超 5s 默认值（GC3b 负载型假红根因）——上调只是给预算，断言零弱化
    { timeout: 30_000 },
    () => {
      const methods = chatMethodsOfIpcBackend();
      expect(methods.length).toBeGreaterThan(0);
      const files = SCAN_DIRS.flatMap((dir) => listFiles(join(SRC, dir)));
      // 每文件只读一次缓存：守卫语义不变，fs 读次数从 方法数×文件数 降为 文件数
      const contents = files.map((file) => readFileSync(file, "utf8"));
      const missing = methods.filter((method) => {
        const called = contents.some((content) => content.includes(method));
        return !called && !(method in EXEMPT);
      });
      expect(missing).toEqual([]);
      // 本任务红线：chatFriendInvite 必须有真实界面调用点，不得进豁免清单
      expect(methods).toContain("chatFriendInvite");
      expect(
        contents.some((content) => content.includes("chatFriendInvite")),
      ).toBe(true);
      // IM-T42 起红线：chatFriendRemove 必须有真实界面调用点，不得进豁免清单
      expect(methods).toContain("chatFriendRemove");
      expect(
        contents.some((content) => content.includes("chatFriendRemove")),
      ).toBe(true);
    },
  );
});
