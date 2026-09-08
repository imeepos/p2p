// borrow 预填交接（§6 交互流第 5 步）：聊天卡片 shareRedeem 成功后把 offer 快照
// （peer/models）暂存于此，/llm-share 视图把 query peer/model 并入后交 BorrowPanel
// 一次性消费。防刷新重放：视图消费即清 query；模块状态仅内存，刷新即失（可接受，
// 候选源降级为自由输入，不再依赖本地 allowlist）。
export interface BorrowPrefill {
  peer: string;
  model: string;
  /** 出借方 offer 快照的模型集（模型候选源，非 allowlist） */
  models: string[];
}

let current: BorrowPrefill | null = null;

export function setBorrowPrefill(prefill: BorrowPrefill): void {
  current = prefill;
}

/** 一次性消费（BorrowPanel 挂载取用后即清，防重复预填） */
export function consumeBorrowPrefill(): BorrowPrefill | null {
  const prefill = current;
  current = null;
  return prefill;
}

/** 探测是否有待消费的预填（不消费）：视图用它决定初始落点 tab */
export function hasBorrowPrefill(): boolean {
  return current !== null;
}

/** 测试/演示复位 */
export function resetBorrowPrefill(): void {
  current = null;
}
