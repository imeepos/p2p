// 概览统计纯函数（tab 化后「概览」首页的数据汇总层）：
// 从流水与净差分组汇总出统计卡数值，不碰 backend、不持状态，便于单测。
import type { LlmBalanceGroup, LlmLedgerEntry } from "./types";

export interface LlmOverviewStats {
  /** 流水总笔数 */
  entriesCount: number;
  /** 全部流水 tokens 合计 */
  totalTokens: number;
  /** 累计出借 tokens（净差分组 lentOut 求和） */
  lentOut: number;
  /** 累计借入 tokens（净差分组 borrowed 求和） */
  borrowed: number;
}

export function summarizeLedger(
  entries: LlmLedgerEntry[],
  groups: LlmBalanceGroup[],
): LlmOverviewStats {
  return {
    entriesCount: entries.length,
    totalTokens: entries.reduce((sum, e) => sum + e.tokens, 0),
    lentOut: groups.reduce((sum, g) => sum + g.lentOut, 0),
    borrowed: groups.reduce((sum, g) => sum + g.borrowed, 0),
  };
}

/** 最近流水：按时间倒序取前 limit 条（概览页只露最近几笔，全量在账本 tab） */
export function recentEntries(entries: LlmLedgerEntry[], limit: number): LlmLedgerEntry[] {
  return [...entries].sort((a, b) => b.ts - a.ts).slice(0, limit);
}
