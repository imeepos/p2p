import { ReceiptVerifyCard } from "./receipt-verify-card";
import { LedgerBalanceCard } from "./ledger-balance";
import { LedgerEntriesCard } from "./ledger-entries";
import type { LlmShareBackend } from "./types";

// 双边账本面板：净差（lender+period 分组方向徽标）/ 流水（三过滤器）/
// 收据验核（lenderPubkey 高级字段）三卡纵向排布，数据互不阻塞。
export function LedgerPanel({ backend }: { backend: LlmShareBackend }) {
  return (
    <div className="flex flex-col gap-3" data-testid="ledger-panel">
      <LedgerBalanceCard backend={backend} />
      <LedgerEntriesCard backend={backend} />
      <ReceiptVerifyCard backend={backend} />
    </div>
  );
}
