// 验收 6（§3.4 表单三律改造项）：endpoint wsUrl 历史值下拉（去重、最近在前）；
// 表单错误全部稳定错误码 + i18n key（逐码断言快照）。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  friends: vi.fn(),
  invites: vi.fn(),
  history: vi.fn(),
  nodeStatus: vi.fn(),
}));

vi.mock("@/lib/ipc", () => ({
  ipc: {
    chatFriendsList: mocks.friends,
    chatInvitesList: mocks.invites,
    chatHistory: mocks.history,
    chatFriendInvite: vi.fn(),
    chatFriendRemove: vi.fn(),
    chatInviteAccept: vi.fn(),
    chatInviteReject: vi.fn(),
    chatInviteCancel: vi.fn(),
    chatSend: vi.fn(),
    groupList: vi.fn().mockResolvedValue([]),
    nodeStatus: mocks.nodeStatus,
    onNodeEvent: () => Promise.resolve(() => {}),
  },
}));

import "@/i18n";
import i18n from "@/i18n";
import type { I18nKey } from "@/i18n/types";
import { MemoryRouter } from "react-router-dom";
import { ConfirmProvider } from "@/components/feedback/confirm-provider";
import { useChatStore } from "@/stores/chat-store";
import { useGroupStore } from "@/stores/group-store";
import { useAcpStore } from "@/acp/acp-store";
import { useEndpointMetaStore } from "@/acp/endpoint-meta";
import { adminEndpointCandidates } from "@/acp/admin-endpoints";

import { EndpointAddDialog } from "./endpoint-add-dialog";

const PEER = "UYJtjuS5i36uXyv74V6aJDHbuShQsFAsZaHaJmRU2pX";

/** Radix 下拉点选（jsdom 需 pointer 桩，与 chat 同款辅助） */
async function pickOption(triggerTestId: string, name: string) {
  fireEvent.pointerDown(screen.getByTestId(triggerTestId), {
    button: 0,
    ctrlKey: false,
    pointerType: "mouse",
  });
  const option = await screen.findByRole("option", { name });
  fireEvent.pointerUp(option, { button: 0, pointerType: "mouse" });
  fireEvent.click(option, { button: 0, pointerType: "mouse" });
}

beforeEach(() => {
  localStorage.clear();
  useEndpointMetaStore.getState().resetForTest();
  mocks.friends.mockReset().mockResolvedValue([]);
  mocks.invites.mockReset().mockResolvedValue([]);
  mocks.history.mockReset().mockResolvedValue([]);
  mocks.nodeStatus.mockReset().mockResolvedValue({
    running: true,
    peerId: PEER,
    listenAddrs: [],
    uptimeSecs: 1,
    startedAtMs: 1,
    config: {},
  });
  useChatStore.setState({
    invites: [],
    friends: [],
    friendsLoaded: true,
    friendsError: null,
  });
  useGroupStore.setState({ groups: [], groupsLoaded: true, selfPeerId: PEER, friends: [], friendsLoaded: true });
  useAcpStore.setState({ saved: [], activeEndpointId: null, phase: "idle" });
  // Radix Select 在 jsdom 下需要指针捕获/滚动桩（官方已知测试前提）
  Object.defineProperty(window.HTMLElement.prototype, "scrollIntoView", {
    configurable: true,
    value: vi.fn(),
  });
  Object.defineProperty(window.HTMLElement.prototype, "hasPointerCapture", {
    configurable: true,
    value: vi.fn(),
  });
  Object.defineProperty(window.HTMLElement.prototype, "releasePointerCapture", {
    configurable: true,
    value: vi.fn(),
  });
});

describe("endpoint wsUrl 历史值下拉（§3.4 三律之三）", () => {
  it("聚焦下拉列出历史保存值：去重、最近保存在前", async () => {
    useAcpStore.setState({
      saved: [
        { wsUrl: "ws://10.0.0.1:9000", token: "t", peer: "", endpointId: "ep-a" },
        { wsUrl: "ws://127.0.0.1:8787", token: "t", peer: "", endpointId: "ep-b" },
        { wsUrl: "ws://10.0.0.1:9000", token: "t", peer: "", endpointId: "ep-c" },
      ],
    });
    render(
      <MemoryRouter>
        <ConfirmProvider>
          <EndpointAddDialog open onOpenChange={() => {}} onSaved={() => {}} />
        </ConfirmProvider>
      </MemoryRouter>,
    );
    await waitFor(() => expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy());
    await pickOption("contacts-endpoint-history", "ws://127.0.0.1:8787");
    // 选择历史值即回填
    expect(
      (screen.getByTestId("contacts-endpoint-wsurl") as HTMLInputElement).value,
    ).toBe("ws://127.0.0.1:8787");
  });
});

describe("表单错误 = 稳定错误码 + i18n key（快照断言）", () => {
  interface Case {
    field: "wsurl" | "peer";
    value: string;
    code: string;
  }
  const CASES: Case[] = [
    { field: "wsurl", value: "", code: "wsUrlRequired" },
    { field: "wsurl", value: "http://127.0.0.1:8787", code: "wsUrlInvalid" },
    { field: "wsurl", value: "not a url", code: "wsUrlInvalid" },
    { field: "peer", value: "mock-peer", code: "peerInvalid" },
    { field: "peer", value: "abc", code: "peerInvalid" },
  ];

  it.each(CASES)("错误码 $code 随字段 $field 稳定输出并经 i18n 渲染", async ({ field, value, code }) => {
    render(
      <MemoryRouter>
        <ConfirmProvider>
          <EndpointAddDialog open onOpenChange={() => {}} onSaved={() => {}} />
        </ConfirmProvider>
      </MemoryRouter>,
    );
    fireEvent.change(screen.getByTestId("contacts-endpoint-" + field), {
      target: { value },
    });
    fireEvent.click(screen.getByTestId("contacts-endpoint-test"));
    const node = await screen.findByTestId("contacts-endpoint-error-" + code);
    // 稳定口径：i18n key 固定为 contacts.endpoint.errors.<code>，文案逐字一致
    expect(node.textContent).toBe(i18n.t(("contacts.endpoint.errors." + code) as I18nKey));
  });
});

describe("endpoint 分享管理（零配置向导）", () => {
  function renderDialog() {
    return render(
      <MemoryRouter>
        <ConfirmProvider>
          <EndpointAddDialog open onOpenChange={() => {}} onSaved={() => {}} />
        </ConfirmProvider>
      </MemoryRouter>,
    );
  }

  it("打开即按 wsUrl 预填管理地址：ws -> http 同 host:port", async () => {
    useAcpStore.setState({
      saved: [],
      draft: { wsUrl: "ws://192.168.1.8:8787", token: "ws-token", peer: "" },
    });
    renderDialog();
    await waitFor(() => expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy());
    expect(
      (screen.getByTestId("contacts-endpoint-adminurl") as HTMLInputElement).value,
    ).toBe("http://192.168.1.8:8787");
  });

  it("空 token 保存放行（先存后连，§3.2 原始口径）：tokenRequired 移至连接路径", async () => {
    useAcpStore.setState({
      saved: [],
      draft: { wsUrl: "ws://127.0.0.1:8787", token: "", peer: "" },
    });
    renderDialog();
    await waitFor(() => expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy());
    fireEvent.click(screen.getByTestId("contacts-endpoint-save"));
    await waitFor(() => expect(useAcpStore.getState().saved.length).toBe(1));
  });

  it("adminUrl 非法保存被拦：adminUrlInvalid 稳定错误码", async () => {
    useAcpStore.setState({
      saved: [],
      draft: { wsUrl: "ws://127.0.0.1:8787", token: "ws-token", peer: "" },
    });
    renderDialog();
    await waitFor(() => expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy());
    fireEvent.change(screen.getByTestId("contacts-endpoint-adminurl"), {
      target: { value: "ftp://bad" },
    });
    fireEvent.click(screen.getByTestId("contacts-endpoint-save"));
    await screen.findByTestId("contacts-endpoint-error-adminUrlInvalid");
  });

  it("填齐管理 Token 保存后：分享管理端点候选立即可用（分享解锁）", async () => {
    useAcpStore.setState({
      saved: [],
      draft: { wsUrl: "ws://127.0.0.1:8787", token: "ws-token", peer: "" },
    });
    renderDialog();
    await waitFor(() => expect(screen.getByTestId("contacts-endpoint-dialog")).toBeTruthy());
    fireEvent.change(screen.getByTestId("contacts-endpoint-admintoken"), {
      target: { value: "admin-token" },
    });
    fireEvent.click(screen.getByTestId("contacts-endpoint-save"));
    await waitFor(() => {
      const cands = adminEndpointCandidates(
        useAcpStore.getState().saved,
        useAcpStore.getState().draft,
      );
      expect(cands).toHaveLength(1);
      expect(cands[0]!.url).toBe("http://127.0.0.1:8787");
      expect(cands[0]!.token).toBe("admin-token");
    });
  });
});
