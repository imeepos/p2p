// 长列表压力测试（react-virtuoso 路径）：5000 条消息下 DOM 节点数有界、
// 滚动窗口移动、加载更早前插锚定、钉底跟随、引用跳转全部保持语义。
// jsdom 无布局，经 VirtuosoMockContext + 几何桩驱动（见 test/jsdom-virt.ts）。
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
import type { ChatMessageJson } from "@/lib/ipc-types";
import i18n from "@/i18n";
import { useChatStore } from "@/stores/chat-store";

import { MESSAGE_VIRTUAL_THRESHOLD, MessageList } from "./message-list";

vi.mock("@/lib/ipc", () => ({ ipc: {} }));

const PEER = "3xY9abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWX";
const TOTAL = 5000;
const ITEM_H = 48;

function text(id: string, content: string, tsMs: number, replyTo?: string): ChatMessageJson {
  return {
    id,
    peer: PEER,
    sender: "me",
    kind: "text",
    tsMs,
    text: content,
    media: null,
    status: "delivered",
    replyTo: replyTo ?? null,
  };
}

function makeMessages(count: number, from = 0): ChatMessageJson[] {
  const base = Date.UTC(2026, 8, 10, 8, 0, 0);
  return Array.from({ length: count }, (_, i) =>
    text(`m-${from + i}`, `消息 ${from + i}`, base + i * 60_000),
  );
}

function Harness({
  messages,
  loadingOlder = false,
  hasMore = true,
  onLoadOlder = () => {},
}: {
  messages: ChatMessageJson[];
  loadingOlder?: boolean;
  hasMore?: boolean;
  onLoadOlder?: () => void;
}): ReactElement {
  return (
    <VirtuosoMockContext.Provider value={VIRT_MOCK_CONTEXT}>
      <div style={{ display: "flex", flexDirection: "column", height: 600 }}>
        <MessageList
          peer={PEER}
          messages={messages}
          loadingOlder={loadingOlder}
          hasMore={hasMore}
          onLoadOlder={onLoadOlder}
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

function scrollerOf(container: HTMLElement): HTMLElement {
  const el = container.querySelector<HTMLElement>("[data-testid='message-scroll']");
  if (!el) throw new Error("虚拟滚动容器未挂载");
  return el;
}

async function renderList(
  messages: ChatMessageJson[],
  props: Partial<Parameters<typeof Harness>[0]> = {},
) {
  let current = messages;
  const view = render(<Harness messages={current} {...props} />);
  const rerenderWith = (
    next: ChatMessageJson[],
    overrides: Partial<Parameters<typeof Harness>[0]> = {},
  ) => {
    current = next;
    view.rerender(<Harness messages={current} {...props} {...overrides} />);
  };
  return { view, rerenderWith, scroller: () => scrollerOf(view.container) };
}

beforeAll(() => {
  installJsdomVirtPolyfill();
});

beforeEach(async () => {
  useChatStore.setState({ messagesByPeer: {}, historyError: {}, olderError: {} });
  await i18n.changeLanguage("zh-CN");
});

describe("MessageList 压力测试（5000 条，react-virtuoso 路径）", () => {
  it("超过阈值走虚拟路径，DOM 气泡数有界", async () => {
    const { view } = await renderList(makeMessages(TOTAL));
    await act(async () => {
      await settleVirt();
    });
    const bubbles = visibleIds(view.container);
    expect(TOTAL).toBeGreaterThan(MESSAGE_VIRTUAL_THRESHOLD);
    expect(bubbles.length).toBeGreaterThan(0);
    expect(bubbles.length).toBeLessThan(100);
    // 初始窗口在顶部：首条可见，末条未挂载
    expect(bubbles[0]).toBe("m-0");
    expect(visibleIds(view.container)).not.toContain(`m-${TOTAL - 1}`);
  });

  it("滚动到底部：窗口移动且节点数仍有界", async () => {
    const { view, scroller } = await renderList(makeMessages(TOTAL));
    scroller().setAttribute("data-scroll-height", String(TOTAL * ITEM_H));
    await act(async () => {
      await settleVirt();
      setScrollTop(scroller(), TOTAL * ITEM_H - 600);
      await settleVirt();
    });
    const ids = visibleIds(view.container);
    expect(ids.length).toBeLessThan(100);
    expect(ids).toContain(`m-${TOTAL - 1}`);
    expect(ids).not.toContain("m-0");
  });

  it("滚动到顶触发加载更早，前插 100 条后视口内容锚定不跳", async () => {
    const messages = makeMessages(TOTAL);
    const onLoadOlder = vi.fn(() => {
      // 模拟 store 异步前插 100 条更早历史（含首条 id 变化）；真实 DOM 的
      // scrollHeight 随内容自动增长，桩需同步更新
      const older = makeMessages(100, TOTAL);
      rerenderWith([...older, ...messages]);
      scroller().setAttribute("data-scroll-height", String((TOTAL + 100) * ITEM_H));
    });
    const { view, rerenderWith, scroller } = await renderList(messages, { onLoadOlder });
    scroller().setAttribute("data-scroll-height", String(TOTAL * ITEM_H));
    await act(async () => {
      await settleVirt();
      setScrollTop(scroller(), 120 * ITEM_H);
      await settleVirt();
    });
    const before = visibleIds(view.container);
    expect(before.length, `before=${before.length} first=${before[0]} st=${scroller().scrollTop}`).toBeGreaterThan(0);

    await act(async () => {
      setScrollTop(scroller(), 0);
      // startReached 走 200ms 节流流，条件轮询避免竞态
      await settleUntil(() => onLoadOlder.mock.calls.length > 0);
    });
    expect(onLoadOlder).toHaveBeenCalled();

    // 前插锚定等价（UX5）：到顶时视口为 m-0..，前插 100 条后同批内容
    // 仍应在视口（内容索引整体 +100，scrollTop 平移 100*48），不跳最新
    const after = visibleIds(view.container);
    expect(after.slice(0, 5)).toEqual(["m-0", "m-1", "m-2", "m-3", "m-4"]);
    expect(after).not.toContain(`m-${TOTAL - 1}`);
    expect(scroller().scrollTop).toBe(100 * ITEM_H);
    expect(before.length).toBeGreaterThan(0);
  });

  it("钉底态下追加新消息自动跟随", async () => {
    const { view, rerenderWith, scroller } = await renderList(makeMessages(TOTAL));
    scroller().setAttribute("data-scroll-height", String(TOTAL * ITEM_H));
    await act(async () => {
      await settleVirt();
      setScrollTop(scroller(), TOTAL * ITEM_H - 600);
      await settleVirt(8);
    });
    const bottomIds = visibleIds(view.container);
    expect(bottomIds.length, `bottomN=${bottomIds.length} st=${scroller().scrollTop}`).toBeGreaterThan(0);
    expect(bottomIds).toContain(`m-${TOTAL - 1}`);

    const beforeTop = scroller().scrollTop;
    await act(async () => {
      rerenderWith([...makeMessages(TOTAL), text(`m-${TOTAL}`, "新消息", Date.now())]);
      scroller().setAttribute("data-scroll-height", String((TOTAL + 1) * ITEM_H));
      await settleVirt(8);
    });
    expect(visibleIds(view.container)).toContain(`m-${TOTAL}`);
    expect(scroller().scrollTop).toBeGreaterThan(beforeTop);
  });

  it("引用跳转：目标不在视口内时滚动定位并高亮", async () => {
    // 当前视口 0..12；引用 4000 条前的 m-3000
    const messages = makeMessages(TOTAL);
    messages[2] = text("m-2", "引用触发", 1000 * 60 * 2, "m-3000");
    const { view, scroller } = await renderList(messages);
    scroller().setAttribute("data-scroll-height", String(TOTAL * ITEM_H));
    await act(async () => {
      await settleVirt();
    });
    expect(visibleIds(view.container)).not.toContain("m-3000");

    // QuoteBlock 点击入口在气泡内；直接走 DOM 事件触发 onQuoteOpen
    const quote = view.container.querySelector("[data-testid='chat-quote-block']");
    expect(quote).not.toBeNull();
    await act(async () => {
      quote!.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await settleVirt(8);
    });
    const ids = visibleIds(view.container);
    expect(ids).toContain("m-3000");
    const target = view.container.querySelector("[data-message-id='m-3000']");
    expect(target?.getAttribute("data-highlighted")).toBe("true");
  });
});
