//! 契约 §16.1 九命令 IPC 面：State 抽取 + 委托 flows（主流程在 flows.rs，
//! tests 直调冒烟不依赖 Tauri runtime）。async 声明使命令进 tauri 异步运行时，
//! 文件 IO 不占主线程；结构化拒绝（borrow rejected / verify FAIL）是业务结果，
//! 照常返回报告非 Err（§16.2.1）。

use tauri::State;

use super::flows;
use super::inputs::{LlmBorrowRequest, LlmLedgerFilter, LlmOfferPublishInput};
use super::views::{
    LlmAllowlistView, LlmBalanceGroup, LlmBorrowReport, LlmLedgerEntry, LlmOfferView,
    LlmReceiptVerifyResult,
};
use super::LlmShareStore;
use crate::state::AppState;

/// llm_share_offer_publish：发布出借声明（签名信封落 offer.json，§16.2.4 唯一合法写路径）。
#[tauri::command]
pub async fn llm_share_offer_publish(
    store: State<'_, LlmShareStore>,
    state: State<'_, AppState>,
    offer: LlmOfferPublishInput,
) -> Result<LlmOfferView, String> {
    flows::offer_publish(&store, &state.config_get(), &offer)
}

/// llm_share_offer_show：当前声明与剩余 TTL（五态）。
#[tauri::command]
pub async fn llm_share_offer_show(store: State<'_, LlmShareStore>) -> Result<LlmOfferView, String> {
    flows::offer_show(&store)
}

/// llm_share_allow_list：白名单清单。
#[tauri::command]
pub async fn llm_share_allow_list(
    store: State<'_, LlmShareStore>,
) -> Result<LlmAllowlistView, String> {
    flows::allow_list(&store)
}

/// llm_share_allow：upsert 白名单（models 缺省=不限模型，§16.1 原话）。
#[tauri::command]
pub async fn llm_share_allow(
    store: State<'_, LlmShareStore>,
    peer_id: String,
    models: Option<Vec<String>>,
    note: Option<String>,
) -> Result<LlmAllowlistView, String> {
    flows::allow(
        &store,
        &peer_id,
        &models.unwrap_or_default(),
        note.as_deref(),
    )
}

/// llm_share_deny：移出白名单（不存在条目显式 Err，默认拒绝语义）。
#[tauri::command]
pub async fn llm_share_deny(
    store: State<'_, LlmShareStore>,
    peer_id: String,
) -> Result<LlmAllowlistView, String> {
    flows::deny(&store, &peer_id)
}

/// llm_share_borrow：借方一次性调用（真实成本动作；maxTokens/targetPeer 必填校验
/// 在 flows::normalize_borrow_request）。
#[tauri::command]
pub async fn llm_share_borrow(
    store: State<'_, LlmShareStore>,
    state: State<'_, AppState>,
    req: LlmBorrowRequest,
) -> Result<LlmBorrowReport, String> {
    flows::borrow(&store, &state.config_get(), req).await
}

/// llm_share_ledger_list：过滤流水明细。
#[tauri::command]
pub async fn llm_share_ledger_list(
    store: State<'_, LlmShareStore>,
    filter: LlmLedgerFilter,
) -> Result<Vec<LlmLedgerEntry>, String> {
    flows::ledger_list(&store, &filter)
}

/// llm_share_ledger_balance：本机净差视图（按 lender+period 切分）。
#[tauri::command]
pub async fn llm_share_ledger_balance(
    store: State<'_, LlmShareStore>,
    state: State<'_, AppState>,
) -> Result<Vec<LlmBalanceGroup>, String> {
    flows::ledger_balance(&store, &state.config_get())
}

/// llm_share_receipt_verify：单笔收据离线验签（FAIL 是业务结果非 Err）。
#[tauri::command]
pub async fn llm_share_receipt_verify(
    store: State<'_, LlmShareStore>,
    state: State<'_, AppState>,
    req_id: String,
    lender_pubkey: Option<String>,
) -> Result<LlmReceiptVerifyResult, String> {
    flows::receipt_verify(
        &store,
        &state.config_get(),
        &req_id,
        lender_pubkey.as_deref(),
    )
}
