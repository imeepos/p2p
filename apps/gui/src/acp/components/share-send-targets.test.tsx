// 分享发送目标组件测试：好友/群多选 → chatSend/groupSend 逐个发送 + 结果摘要。
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.stubEnv("VITE_MOCK_IPC", "1");

const { ShareSendTargets } = await import("./share-send-targets");
const { mockBackend } = await import("@/lib/mock-ipc");
await import("@/i18n");

const LINK = "dsh-acp-share://v1?peer=peerA&token=" + "f".repeat(32);

const FRIENDS = [
  { peerId: "peerAaaaaaaaaa", nickname: "小明", addrs: [] },
  { peerId: "peerBbbbbbbbbb", nickname: "小红", addrs: [] },
];
const GROUPS = [
  {
    groupId: "g-1",
    name: "p2p 群",
    owner: "peerOwner",
    members: ["peerOwner"],
    rev: 1,
    state: "active" as const,
    tsMs: 0,
  },
];

const sendSpy = vi.fn();
const groupSpy = vi.fn();

beforeEach(() => {
  sendSpy.mockClear();
  groupSpy.mockClear();
  vi.spyOn(mockBackend, "chatFriendsList").mockResolvedValue(FRIENDS);
  vi.spyOn(mockBackend, "groupList").mockResolvedValue(GROUPS);
  vi.spyOn(mockBackend, "chatSend").mockImplementation(sendSpy);
  vi.spyOn(mockBackend, "groupSend").mockImplementation(groupSpy);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ShareSendTargets 发送目标区", () => {
  it("好友与群聊列出且带命名空间勾选框", async () => {
    render(<ShareSendTargets link={LINK} />);
    expect(await screen.findByText("小明")).toBeTruthy();
    expect(screen.getByText("p2p 群")).toBeTruthy();
    expect(screen.getByTestId("acp-share-target-check-friend:peerAaaaaaaaaa")).toBeTruthy();
    expect(screen.getByTestId("acp-share-target-check-group:g-1")).toBeTruthy();
  });

  it("勾选好友与群聊后发送：文本消息逐个走 chatSend/groupSend，链接即正文", async () => {
    render(<ShareSendTargets link={LINK} />);
    await screen.findByText("小明");
    fireEvent.click(screen.getByTestId("acp-share-target-check-friend:peerAaaaaaaaaa"));
    fireEvent.click(screen.getByTestId("acp-share-target-check-group:g-1"));
    fireEvent.click(screen.getByTestId("acp-share-send-targets"));
    await waitFor(() => {
      expect(sendSpy).toHaveBeenCalledWith("peerAaaaaaaaaa", "text", LINK);
      expect(groupSpy).toHaveBeenCalledWith("g-1", "text", LINK);
    });
    expect(await screen.findByTestId("acp-share-targets-summary")).toBeTruthy();
  });
});
