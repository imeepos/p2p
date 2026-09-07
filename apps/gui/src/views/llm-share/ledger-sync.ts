// 借用入账后账本卡联动刷新（R2-01）：域内轻量 pub-sub。borrow 产生真实
// 入账（done/stream_broken，appended=true）时发信号，净差/流水卡订阅重拉；
// 跨面板通信用模块级事件而非提升状态，避免四面板整体重渲染。
type Listener = () => void;

const listeners = new Set<Listener>();

export function notifyLedgerMutated(): void {
  for (const listener of [...listeners]) listener();
}

export function subscribeLedgerMutated(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
