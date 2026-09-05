import { fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ChatFriendJson,
  ChatMessageJson,
  GroupJson,
  NodeEventHandler,
} from "@/lib/ipc-types";
import {
  ENDPOINT_ID,
  GROUP_ID,
  PEER,
  PEER_B,
  installMatchMedia,
  resetViewport,
  seedAll,
  renderAt,
  setViewportNarrow,
  type MediaMocks,
} from "@/test/chat-page-fixtures";
import type { Mock } from "vitest";

import "@/i18n";

// P1 聚合聊天验收（§2.1 <768 单栏互斥、§2.4 会话搜索）。
// 混排与深链见 chat-page.test.tsx。

const { mocks } = vi.hoisted(() => ({
  mocks: {
    friends: vi.fn<() => Promise<ChatFriendJson[]>>(),
    history: vi.fn<(peer: string) => Promise<ChatMessageJson[]>>(),
    send: vi.fn(),
    groupList: vi.fn<() => Promise<GroupJson[]>>(),
    groupHistory: vi.fn<() => Promise<never[]>>(),
    invites: vi.fn<() => Promise<never[]>>(),
    nodeStatus: vi.fn(),
    handlers: [] as NodeEventHandler[],
  },
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatHistory: mocks.history,
    chatSend: mocks.send,
    groupList: mocks.groupList,
    groupHistory: mocks.groupHistory,
    chatInvitesList: mocks.invites,
    nodeStatus: mocks.nodeStatus,
    onNodeEvent: (handler: NodeEventHandler) => {
      mocks.handlers.push(handler);
      return Promise.resolve(() => {});
    },
  },
}));

beforeEach(() => {
  for (const fn of Object.values(mocks)) {
    if (typeof fn === "function") (fn as Mock).mockReset();
  }
  mocks.handlers.length = 0;
  installMatchMedia();
  seedAll(mocks as unknown as MediaMocks);
});

afterEach(() => {
  resetViewport();
});

describe("<768 单栏互斥（§2.1，jsdom 视口模拟）", () => {
  it("窄屏默认只显列表；选中切入记录；返回按钮回列表", async () => {
    setViewportNarrow(true);
    renderAt();
    await screen.findByTestId("conversation-row-friend-" + PEER);
    // 默认显列表，记录区不可见
    expect(screen.getByTestId("chat-list-pane")).toBeTruthy();
    expect(screen.queryByTestId("chat-conversation-pane")).toBeNull();

    fireEvent.click(screen.getByTestId("conversation-row-friend-" + PEER));
    await waitFor(() => expect(screen.getByTestId("chat-input")).toBeTruthy());
    // 互斥：记录区可见，列表不可见；返回按钮在记录区左上
    expect(screen.getByTestId("chat-conversation-pane")).toBeTruthy();
    expect(screen.queryByTestId("chat-list-pane")).toBeNull();
    expect(screen.getByTestId("chat-back")).toBeTruthy();

    fireEvent.click(screen.getByTestId("chat-back"));
    await waitFor(() => expect(screen.getByTestId("chat-list-pane")).toBeTruthy());
    expect(screen.queryByTestId("chat-input")).toBeNull();
  });

  it("宽屏双栏并存；窄屏选中后转宽恢复双栏且返回按钮消失", async () => {
    setViewportNarrow(true);
    renderAt();
    await screen.findByTestId("conversation-row-friend-" + PEER);
    fireEvent.click(screen.getByTestId("conversation-row-friend-" + PEER));
    await waitFor(() => expect(screen.getByTestId("chat-back")).toBeTruthy());

    setViewportNarrow(false);
    await waitFor(() => expect(screen.getByTestId("chat-list-pane")).toBeTruthy());
    expect(screen.queryByTestId("chat-back")).toBeNull();
    expect(screen.getByTestId("chat-conversation-pane")).toBeTruthy();
  });

  it("深链 + 窄屏：直接切入记录区（列表隐藏），返回后回列表", async () => {
    setViewportNarrow(true);
    renderAt("/chat?agent=" + ENDPOINT_ID);
    await screen.findByTestId("agent-connect-card");
    expect(screen.queryByTestId("chat-list-pane")).toBeNull();
    fireEvent.click(screen.getByTestId("chat-back"));
    await waitFor(() => expect(screen.getByTestId("chat-list-pane")).toBeTruthy());
  });
});

describe("会话搜索（§2.4）", () => {
  it("标题子串过滤；ID 前缀命中；无结果空态；清空恢复", async () => {
    renderAt();
    await screen.findByTestId("conversation-row-friend-" + PEER);
    const input = screen.getByTestId("conversation-search") as HTMLInputElement;

    // 标题子串：命中「小圆」，排除群与 agent
    fireEvent.change(input, { target: { value: "小圆" } });
    expect(screen.getByTestId("conversation-row-friend-" + PEER)).toBeTruthy();
    expect(screen.queryByTestId("conversation-row-group-" + GROUP_ID)).toBeNull();
    expect(screen.queryByTestId("conversation-row-agent-" + ENDPOINT_ID)).toBeNull();

    // 备注命中：PEER_B note=备注甲
    fireEvent.change(input, { target: { value: "备注甲" } });
    expect(screen.getByTestId("conversation-row-friend-" + PEER_B)).toBeTruthy();

    // ID 前缀：群 UUID 前缀命中群条目
    fireEvent.change(input, { target: { value: GROUP_ID.slice(0, 8) } });
    expect(screen.getByTestId("conversation-row-group-" + GROUP_ID)).toBeTruthy();
    expect(screen.queryByTestId("conversation-row-friend-" + PEER)).toBeNull();

    // 无结果空态
    fireEvent.change(input, { target: { value: "不存在的会话" } });
    expect(screen.getByText("无匹配会话")).toBeTruthy();

    // 清空恢复全量
    fireEvent.change(input, { target: { value: "" } });
    expect(screen.getByTestId("conversation-row-agent-" + ENDPOINT_ID)).toBeTruthy();
  });
});