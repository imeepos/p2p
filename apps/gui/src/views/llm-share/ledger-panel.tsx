import { useState } from "react";

import { ReceiptVerifyCard } from "./receipt-verify-card";
import { LedgerBalanceCard } from "./ledger-balance";
import { LedgerEntriesCard } from "./ledger-entries";
import type { LlmShareBackend } from "./types";

// 双边账本面板：净差（lender+period 分组方向徽标）/ 流水（三过滤器）/
// 收据验核（lenderPubkey 高级字段）三卡纵向排布，数据互不阻塞。
// 「查询」联动净差（R2-01）：流水卡查询完成后经 reloadSignal 触发净差重拉。
export function LedgerPanel({ backend }: { backend: LlmShareBackend }) {
  const [balanceReload, setBalanceReload] = useState(0);
  return (
    <div className="flex flex-col gap-3" data-testid="ledger-panel">
      <LedgerBalanceCard backend={backend} reloadSignal={balanceReload} />
      <LedgerEntriesCard
        backend={backend}
        onQueried={() => setBalanceReload((n) => n + 1)}
      />
      <ReceiptVerifyCard backend={backend} />
    </div>
  );
}
