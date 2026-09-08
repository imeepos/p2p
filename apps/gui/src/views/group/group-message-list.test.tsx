// UX5 回归：群历史向上翻页前插补偿。WebKit 无滚动锚定，前插更早历史后
// scrollTop 必须平移高度增量，视口锚定不跳。jsdom 无布局引擎，以高度桩
// 模拟 scrollHeight 前后变化做数值断言。
import { fireEvent, render, screen } from "@testing-library/react";
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import type { ChatFriendJson, GroupMessageJson } from "@/lib/ipc-types";

import "@/i18n";

import { GroupMessageList } from "./group-message-list";

const SELF = "self-peer-id";

function groupMsg(id: string, senderId: string, text: string): GroupMessageJson {
  return {
    id,
    groupId: "g1",
    senderId,
    kind: "text",
    tsMs: 0,
    text,
    media: null,
    status: "delivered",
    acks: [],
    replyTo: null,
  };
}

function msgWithStatus(
  id: string,
  status: GroupMessageJson["status"],
  acks: string[] = [],
): GroupMessageJson {
  return { ...groupMsg(id, SELF, "我的消息"), status, acks };
}

const BASE = [
  groupMsg("m1", "peer-a", "一"),
  groupMsg("m2", SELF, "二"),
  groupMsg("m3", "peer-a", "三"),
];

function listElement(
  messages: GroupMessageJson[],
  overrides: { onLoadOlder?: () => void; hasMore?: boolean; onRetry?: (m: GroupMessageJson) => void } = {},
) {
  const props = {
    groupId: "g1",
    messages,
    selfPeerId: SELF,
    friends: [] as ChatFriendJson[],
    totalRecipients: 2,
    loadingOlder: false,
    hasMore: true,
    historyError: null,
    onLoadOlder: vi.fn(),
    onRetryHistory: () => Promise.resolve(undefined),
    onCancelPending: () => {},
    ...overrides,
  };
  return <GroupMessageList {...props} />;
}

const heightMock = { value: 0 };
const originalDescriptor = Object.getOwnPropertyDescriptor(
  HTMLElement.prototype,
  "scrollHeight",
);

beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
    configurable: true,
    get: () => heightMock.value,
  });
});

afterAll(() => {
  if (originalDescriptor) {
    Object.defineProperty(HTMLElement.prototype, "scrollHeight", originalDescriptor);
  } else {
    Reflect.deleteProperty(HTMLElement.prototype, "scrollHeight");
  }
});

beforeEach(() => {
  heightMock.value = 0;
});

describe("群历史前插滚动补偿（UX5）", () => {
  it("向上翻页前插后 scrollTop 平移高度增量，视口锚定不跳", () => {
    heightMock.value = 1000;
    const onLoadOlder = vi.fn();
    const { rerender } = render(listElement(BASE, { onLoadOlder }));
    const el = screen.getByTestId("group-message-scroll");
    // 初始钉底后向上翻页：接近顶部触发加载更早
    el.scrollTop = 30;
    fireEvent.scroll(el);
    expect(onLoadOlder).toHaveBeenCalledTimes(1);
    // 前插 m0：内容高度 1000 -> 1600，视口应平移 +600（30 -> 630）
    heightMock.value = 1600;
    rerender(listElement([groupMsg("m0", "peer-a", "零"), ...BASE], { onLoadOlder }));
    expect(el.scrollTop).toBe(630);
    expect(onLoadOlder).toHaveBeenCalledTimes(1);
  });

  it("底部追加新消息（非前插）：已离开底部时不补偿也不强制跳底", () => {
    heightMock.value = 1000;
    const { rerender } = render(listElement(BASE));
    const el = screen.getByTestId("group-message-scroll");
    el.scrollTop = 30;
    fireEvent.scroll(el);
    heightMock.value = 1600;
    rerender(listElement([...BASE, groupMsg("m4", SELF, "新消息")]));
    expect(el.scrollTop).toBe(30);
  });

  it("钉底态下新消息到达仍自动跟随到底", () => {
    heightMock.value = 1000;
    const { rerender } = render(listElement(BASE));
    const el = screen.getByTestId("group-message-scroll");
    el.scrollTop = 980;
    fireEvent.scroll(el);
    heightMock.value = 1600;
    rerender(listElement([...BASE, groupMsg("m4", SELF, "新消息")]));
    expect(el.scrollTop).toBe(1600);
  });
});

describe("群发送状态反馈链（W1-01）", () => {
  it("pending 显发送中而非已送达 0/n；failed 显失败并给重发入口", () => {
    const onRetry = vi.fn();
    render(
      listElement([msgWithStatus("m-p", "pending"), msgWithStatus("m-f", "failed")], {
        hasMore: false,
        onRetry,
      }),
    );
    const statuses = screen.getAllByTestId("message-status");
    expect(statuses[0].textContent).toContain("发送中");
    expect(statuses[0].textContent).not.toContain("已送达");
    expect(statuses[1].textContent).toContain("失败");
    expect(statuses[1].textContent).not.toContain("已送达");
    fireEvent.click(screen.getByTestId("message-retry-m-f"));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it("delivered 仍显送达计数 k/n", () => {
    render(
      listElement([msgWithStatus("m-d", "delivered", ["peer-a"])], { hasMore: false }),
    );
    expect(screen.getByTestId("message-status").textContent).toContain("已送达 1/2");
  });
});

describe("群消息流布局契约（横向滚动零容忍）", () => {
  it("滚动域 overflow-x-hidden + 连续消息纵向间隔", () => {
    render(listElement(BASE));
    expect(screen.getByTestId("group-message-scroll").className).toContain(
      "overflow-x-hidden",
    );
    expect(screen.getByTestId("group-message-column").className).toContain(
      "gap-y-2.5",
    );
  });
});
