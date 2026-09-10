import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { Virtuoso, type VirtuosoHandle } from "react-virtuoso";

import { LoadingHistoryHint, OlderErrorBanner } from "./history-notices";

// 长消息流虚拟化（react-virtuoso）：动态行高 + 钉底跟随 + firstItemIndex
// 前插锚定 + startReached 触发加载更早。短列表（≤阈值）仍走普通渲染路径，
// 见 message-list 的阈值切换。

export const FIRST_ITEM_INDEX_BASE = 1_000_000;

export interface MessageFlowHandle {
  scrollToIndex: (index: number) => void;
}

export interface VirtualMessageFlowProps<T> {
  items: T[];
  itemId: (item: T) => string;
  scrollerTestId: string;
  canLoadOlder: boolean;
  loadingOlder: boolean;
  onTopReached: () => void;
  /** 顶部信号区（更早失败横幅 / 加载中提示），随 Header 挂在滚动区顶端 */
  header?: ReactNode;
  /** 老失败信号（IM-T50 语义）：置顶且可重试；群流无该信号 */
  olderError?: string | null;
  onRetryOlder?: () => Promise<unknown>;
  renderItem: (item: T, prevItem: T | null, highlighted: boolean) => ReactNode;
  highlightId: string | null;
}

// 前插锚定：firstItemIndex 随前插递减（官方语义），渲染期推导保证与 items
// 同一 commit 提交给 virtuoso，视口内容等价（替代旧 UX5 手工 scrollTop 补偿）。
function usePrependAnchor<T>(items: T[], itemId: (item: T) => string) {
  const [anchor, setAnchor] = useState({
    firstId: items.length > 0 ? itemId(items[0]) : null,
    firstItemIndex: FIRST_ITEM_INDEX_BASE,
  });
  const firstId = items.length > 0 ? itemId(items[0]) : null;
  if (firstId !== anchor.firstId) {
    // 整体替换（切换会话）时 oldPos<0：保持 firstItemIndex 不变由 remount 兜底
    const oldPos = anchor.firstId === null
      ? -1
      : items.findIndex((item) => itemId(item) === anchor.firstId);
    const next = oldPos > 0
      ? anchor.firstItemIndex - oldPos
      : anchor.firstItemIndex;
    setAnchor({ firstId, firstItemIndex: next });
  }
  return anchor.firstItemIndex;
}

function VirtualMessageFlowInner<T>(
  {
    items,
    itemId,
    scrollerTestId,
    canLoadOlder,
    loadingOlder,
    onTopReached,
    header,
    olderError,
    onRetryOlder,
    renderItem,
    highlightId,
  }: VirtualMessageFlowProps<T>,
  ref: React.Ref<MessageFlowHandle>,
) {
  const virtuosoRef = useRef<VirtuosoHandle | null>(null);
  const atBottomRef = useRef(true);
  const lastIdRef = useRef<string | null>(items.length > 0 ? itemId(items[items.length - 1]) : null);
  const firstItemIndex = usePrependAnchor(items, itemId);

  useImperativeHandle(
    ref,
    () => ({
      scrollToIndex: (index: number) =>
        virtuosoRef.current?.scrollToIndex({ index, behavior: "auto" }),
    }),
    [],
  );

  // 尾部追加跟随（应用侧钉底）：atBottom 时新消息到达滚到底。
  // 不用 followOutput：其对「任意 size 增大」的自动回底会与前插锚定打架。
  useEffect(() => {
    const lastId = items.length > 0 ? itemId(items[items.length - 1]) : null;
    const prev = lastIdRef.current;
    lastIdRef.current = lastId;
    if (prev === null || lastId === null || lastId === prev) return;
    if (atBottomRef.current) {
      virtuosoRef.current?.scrollToIndex({ index: "LAST", align: "end", behavior: "auto" });
    }
  }, [items, itemId]);

  // 挂载落底（等价 initialTopMostItemIndex）：打开会话直接定位最新消息。
  // 不用 initialTopMostItemIndex 的原因：其初始 listState 推导在
  // sizeTree 未就绪时会给出越界窗口（jsdom mock context 即此形态）；
  // 命令式 scrollToIndex 走统一的滚动回路，两个环境行为一致。
  const mountLandedRef = useRef(false);
  useEffect(() => {
    if (mountLandedRef.current || items.length === 0) return;
    mountLandedRef.current = true;
    virtuosoRef.current?.scrollToIndex({ index: "LAST", align: "end", behavior: "auto" });
    // 仅挂载执行一次；后续追加由上方跟随 effect 接管
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleStartReached = useCallback(() => {
    if (canLoadOlder && !loadingOlder) onTopReached();
  }, [canLoadOlder, loadingOlder, onTopReached]);

  const handleAtBottomChange = useCallback((atBottom: boolean) => {
    atBottomRef.current = atBottom;
  }, []);

  const headerNode = useMemo(
    () => (
      <>
        {olderError && onRetryOlder ? (
          <OlderErrorBanner detail={olderError} onRetry={onRetryOlder} />
        ) : null}
        {loadingOlder ? <LoadingHistoryHint /> : null}
        {header}
      </>
    ),
    [olderError, loadingOlder, header, onRetryOlder],
  );

  const components = useMemo(() => ({ Header: () => headerNode }), [headerNode]);

  const itemContent = useCallback(
    (index: number) => {
      // itemContent 收到的是绝对索引（position + firstItemIndex），换算回数组位
      const position = index - firstItemIndex;
      const item = items[position];
      const prev = position > 0 ? items[position - 1] : null;
      if (item === undefined) return null;
      return (
        <div className="pb-2.5">
          {renderItem(item, prev, highlightId === itemId(item))}
        </div>
      );
    },
    [items, firstItemIndex, renderItem, highlightId, itemId],
  );

  return (
    <Virtuoso
      ref={virtuosoRef}
      data-testid={scrollerTestId}
      className="scroll-slim min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-4 py-3"
      totalCount={items.length}
      firstItemIndex={firstItemIndex}
      startReached={handleStartReached}
      atBottomStateChange={handleAtBottomChange}
      components={components}
      increaseViewportBy={{ top: 240, bottom: 240 }}
      itemContent={itemContent}
    />
  );
}

export const VirtualMessageFlow = forwardRef(VirtualMessageFlowInner) as
  <T>(
    props: VirtualMessageFlowProps<T> & { ref?: React.Ref<MessageFlowHandle> },
  ) => React.ReactElement | null;
