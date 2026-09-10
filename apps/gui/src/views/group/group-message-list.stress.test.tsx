// 群消息流压力测试（react-virtuoso 路径）：5000 条群消息下 DOM 节点数有界、
// 昵称/送达计数渲染、滚动窗口移动、加载更早前插锚定、钉底跟随。
import { act, render } from "@testing-library/react";
import type { ReactElement } from "react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { VirtuosoMockContext } from "react-virtuoso";

import {
  VIRT_MOCK_CONTEXT,
  installJsdomVirtPolyfill,
  setScrollTop,
  settleUntil,
  settleVirt,
} from "@/test/jsdom-virt";
import type { ChatFriendJson, GroupMessageJson } from "@/lib/ipc-types";
import i18n from "@/i18n";

import { GroupMessageList } from "./group-message-list";

vi.mock("@/lib/ipc", () => ({ ipc: {} }));

const GROUP = "grp-stress";
const TOTAL = 5000;
const ITEM_H = 48;

function friend(peerId: string, nickname: string): ChatFriendJson {
  return {
    peerId,
    kind: "friend",
    nickname,
    avatar: null,
    online: true,
    unread: 0,
    lastMessage: null,
    agent: null,
  } as unknown as ChatFriendJson;
}

function groupText(id: string, text: string, tsMs: number): GroupMessageJson {
  return {
    id,
    groupId: GROUP,
    senderId: "peer-alice",
    kind: "text",
    tsMs,
    text,
    media: null,
    status: "sent",
    acks: ["peer-bob"],
    replyTo: null,
  } as unknown as GroupMessageJson;
}

function makeMessages(count: number, from = 0): GroupMessageJson[] {
  const base = Date.UTC(2026, 8, 10, 8, 0, 0);
  return Array.from({ length: count }, (_, i) =>
    groupText(`m-${from + i}`, `群消息 ${from + i}`, base + i * 60_000),
  );
}

function Harness({
  messages,
  loadingOlder = false,
  hasMore = true,
  onLoadOlder = () => {},
}: {
  messages: GroupMessageJson[];
  loadingOlder?: boolean;
  hasMore?: boolean;
  onLoadOlder?: () => void;
}): ReactElement {
  return (
    <VirtuosoMockContext.Provider value={VIRT_MOCK_CONTEXT}>
      <div style={{ display: "flex", flexDirection: "column", height: 600 }}>
        <GroupMessageList
          groupId={GROUP}
          messages={messages}
          selfPeerId="self-peer"
          friends={[friend("peer-alice", "Alice"), friend("peer-bob", "Bob")]}
          totalRecipients={2}
          loadingOlder={loadingOlder}
          hasMore={hasMore}
          historyError={null}
          onLoadOlder={onLoadOlder}
          onRetryHistory={() => Promise.resolve()}
          onCancelPending={() => {}}
        />
      </div>
    </VirtuosoMockContext.Provider>
  );
}

function visibleIds(container: HTMLElement): string[] {
  return [...container.querySelectorAll("[data-message-id]")].map(
    (el) => el.getAttribute("data-message-id") as string,
  );
}

beforeAll(() => {
  installJsdomVirtPolyfill();
});

beforeEach(async () => {
  await i18n.changeLanguage("zh-CN");
});

describe("GroupMessageList 压力测试（5000 条，react-virtuoso 路径）", () => {
  it("超过阈值走虚拟路径，DOM 气泡数有界且昵称渲染", async () => {
    const view = render(<Harness messages={makeMessages(TOTAL)} />);
    await act(async () => {
      await settleVirt();
    });
    const ids = visibleIds(view.container);
    expect(ids.length).toBeGreaterThan(0);
    expect(ids.length).toBeLessThan(100);
    expect(view.getAllByText("Alice").length).toBeGreaterThan(0);
    expect(view.queryByText(`群消息 ${TOTAL - 1}`)).not.toBeInTheDocument();
  });

  it("滚动到底部：最新消息可见且节点数有界", async () => {
    const view = render(<Harness messages={makeMessages(TOTAL)} />);
    const scroller = () =>
      view.container.querySelector<HTMLElement>("[data-testid='group-message-scroll']")!;
    scroller().setAttribute("data-scroll-height", String(TOTAL * ITEM_H));
    const withSelf = [
      ...makeMessages(TOTAL - 1),
      { ...groupText(`m-${TOTAL - 1}`, "本机消息", 0), senderId: "self-peer" },
    ];
    view.rerender(<Harness messages={withSelf} />);
    await act(async () => {
      await settleVirt();
      setScrollTop(scroller(), TOTAL * ITEM_H - 600);
      await settleVirt();
    });
    const ids = visibleIds(view.container);
    expect(ids.length).toBeLessThan(100);
    expect(ids).toContain(`m-${TOTAL - 1}`);
    expect(view.getByText("已送达 1/2")).toBeInTheDocument();
  });

  it("滚动到顶触发加载更早，前插 100 条后视口内容锚定不跳", async () => {
    const messages = makeMessages(TOTAL);
    const onLoadOlder = vi.fn(() => {
      const older = makeMessages(100, TOTAL);
      view.rerender(
        <Harness
          messages={[...older, ...messages]}
          onLoadOlder={onLoadOlder}
        />,
      );
      scroller().setAttribute("data-scroll-height", String((TOTAL + 100) * ITEM_H));
    });
    const view = render(<Harness messages={messages} onLoadOlder={onLoadOlder} />);
    const scroller = () =>
      view.container.querySelector<HTMLElement>("[data-testid='group-message-scroll']")!;
    scroller().setAttribute("data-scroll-height", String(TOTAL * ITEM_H));
    await act(async () => {
      await settleVirt();
      setScrollTop(scroller(), 0);
      await settleUntil(() => onLoadOlder.mock.calls.length > 0);
    });
    expect(onLoadOlder).toHaveBeenCalled();
    const after = visibleIds(view.container);
    expect(after.slice(0, 5)).toEqual(["m-0", "m-1", "m-2", "m-3", "m-4"]);
    expect(scroller().scrollTop).toBe(100 * ITEM_H);
  });

  it("钉底态下追加新消息自动跟随", async () => {
    const view = render(<Harness messages={makeMessages(TOTAL)} />);
    const scroller = () =>
      view.container.querySelector<HTMLElement>("[data-testid='group-message-scroll']")!;
    scroller().setAttribute("data-scroll-height", String(TOTAL * ITEM_H));
    await act(async () => {
      await settleVirt();
      setScrollTop(scroller(), TOTAL * ITEM_H - 600);
      await settleVirt(8);
    });
    expect(visibleIds(view.container)).toContain(`m-${TOTAL - 1}`);

    const beforeTop = scroller().scrollTop;
    await act(async () => {
      view.rerender(
        <Harness
          messages={[...makeMessages(TOTAL), groupText(`m-${TOTAL}`, "新消息", Date.now())]}
        />,
      );
      scroller().setAttribute("data-scroll-height", String((TOTAL + 1) * ITEM_H));
      await settleVirt(8);
    });
    expect(visibleIds(view.container)).toContain(`m-${TOTAL}`);
    expect(scroller().scrollTop).toBeGreaterThan(beforeTop);
  });
});
