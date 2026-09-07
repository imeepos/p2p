import { invoke } from "@tauri-apps/api/core";

import { useMockIpc } from "@/lib/ipc";

import type {
  LlmAllowlistView,
  LlmBalanceGroup,
  LlmBorrowReport,
  LlmLedgerEntry,
  LlmOfferView,
  LlmProviderView,
  LlmReceiptVerifyResult,
  LlmServeStatus,
  LlmShareBackend,
  LlmShareCreateResult,
  LlmShareEntry,
  LlmShareRedeemResult,
} from "./types";

// llm-share 数据接缝（契约 §16.1 九命令，命令名逐字 snake_case）：live 分支
// 直连 src-tauri 封装；mock 分支仅 VITE_MOCK_IPC=1 动态装载（同 lib/ipc 防泄
// 漏模式，no-mock-in-build 门禁保证不进产物）。lib/** 归 LSG2，待其 IPC 面落地
// 后由路由挂载点换接线，本接缝类型即跨轨会签对照物。
const liveBackend: LlmShareBackend = {
  offerPublish: (offer) => invoke<LlmOfferView>("llm_share_offer_publish", { offer }),
  offerShow: () => invoke<LlmOfferView>("llm_share_offer_show"),
  allowList: () => invoke<LlmAllowlistView>("llm_share_allow_list"),
  allow: (req) => invoke<LlmAllowlistView>("llm_share_allow", { req }),
  deny: (peerId) => invoke<LlmAllowlistView>("llm_share_deny", { peerId }),
  borrow: (req) => invoke<LlmBorrowReport>("llm_share_borrow", { req }),
  ledgerList: (filter) =>
    invoke<LlmLedgerEntry[]>("llm_share_ledger_list", { filter: filter ?? {} }),
  ledgerBalance: () => invoke<LlmBalanceGroup[]>("llm_share_ledger_balance"),
  receiptVerify: (req) =>
    invoke<LlmReceiptVerifyResult>("llm_share_receipt_verify", { req }),
  // 契约 §16.6 v13 加法（8 条）：live 分支逐字 snake_case invoke；运行期
  // 后端命令由 W3 落地，测试全走 mock 分支。
  providerList: () => invoke<{ providers: LlmProviderView[] }>("llm_share_provider_list"),
  providerSave: (config) => invoke<LlmProviderView>("llm_share_provider_save", { config }),
  providerRemove: (providerId) => invoke<{ removed: true }>("llm_share_provider_remove", { providerId }),
  shareCreate: (req) => invoke<LlmShareCreateResult>("llm_share_share_create", { req }),
  shareList: () => invoke<{ shares: LlmShareEntry[] }>("llm_share_share_list"),
  shareRevoke: (shareId) => invoke<{ revoked: true }>("llm_share_share_revoke", { shareId }),
  shareRedeem: (link) => invoke<LlmShareRedeemResult>("llm_share_share_redeem", { link }),
  serveStatus: () => invoke<LlmServeStatus>("llm_share_serve_status"),
};

const backend: LlmShareBackend = useMockIpc
  ? (await import("./mock-backend")).makeLlmShareMockBackend()
  : liveBackend;

export function resolveLlmShareBackend(): LlmShareBackend {
  return backend;
}
