//! 契约 §16.1 九命令 IPC 面：State 抽取 + 委托 flows（主流程在 flows.rs，
//! tests 直调冒烟不依赖 Tauri runtime）。async 声明使命令进 tauri 异步运行时，
//! 文件 IO 不占主线程；结构化拒绝（borrow rejected / verify FAIL）是业务结果，
//! 照常返回报告非 Err（§16.2.1）。

use p2p_cli::llm_share::provider::ProviderView;
use p2p_cli::llm_share::share::{ShareCreateReport, ShareListReport, ShareRevokeReport};
use p2p_cli::llm_share::share_redeem::RedeemOutcome;
use tauri::State;

use super::flows;
use super::flows_share;
use super::inputs::{
    LlmBorrowRequest, LlmLedgerFilter, LlmOfferPublishInput, LlmProviderSaveInput,
    LlmShareCreateInput,
};
use super::serve::LlmServeStatus;
use super::share_views::{LlmProviderListView, LlmProviderRemoveView};
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

/// llm_share_allow：upsert 白名单（models 缺省=不限模型，§16.1 原话；
/// source/expires_at 透传缺省 None，§16.6 v13 语义不变）。
#[tauri::command]
pub async fn llm_share_allow(
    store: State<'_, LlmShareStore>,
    peer_id: String,
    models: Option<Vec<String>>,
    note: Option<String>,
    source: Option<String>,
    expires_at: Option<u64>,
) -> Result<LlmAllowlistView, String> {
    flows::allow(
        &store,
        &peer_id,
        &models.unwrap_or_default(),
        note.as_deref(),
        source.as_deref(),
        expires_at,
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

// ===== 契约 §16.6 v13 加法（8 条，W3）：provider×3 + share×4 + serve_status =====

/// llm_share_provider_save：apiKey 明文仅经 IPC 入参落 0600 密钥文件。
#[tauri::command]
pub async fn llm_share_provider_save(
    store: State<'_, LlmShareStore>,
    config: LlmProviderSaveInput,
) -> Result<ProviderView, String> {
    flows_share::provider_save(&store, config)
}

/// llm_share_provider_list：apiKey 只出掩码；损坏存档显式 Err 不静默回空。
#[tauri::command]
pub async fn llm_share_provider_list(
    store: State<'_, LlmShareStore>,
) -> Result<LlmProviderListView, String> {
    flows_share::provider_list(&store)
}

/// llm_share_provider_remove：不存在=显式 Err；级联删密钥文件。
#[tauri::command]
pub async fn llm_share_provider_remove(
    store: State<'_, LlmShareStore>,
    provider_id: String,
) -> Result<LlmProviderRemoveView, String> {
    flows_share::provider_remove(&store, &provider_id)
}

/// llm_share_share_create：链接生成（addrs 取运行中节点监听地址，未运行为空表）。
#[tauri::command]
pub async fn llm_share_share_create(
    store: State<'_, LlmShareStore>,
    state: State<'_, AppState>,
    req: LlmShareCreateInput,
) -> Result<ShareCreateReport, String> {
    let status = state.status().await;
    let addrs = if status.running {
        status.listen_addrs
    } else {
        Vec::new()
    };
    flows_share::share_create(&store, &state.config_get(), addrs, req)
}

/// llm_share_share_list：脱敏台账（status 推导，永不含 token/明文 key）。
#[tauri::command]
pub async fn llm_share_share_list(
    store: State<'_, LlmShareStore>,
) -> Result<ShareListReport, String> {
    flows_share::share_list(&store)
}

/// llm_share_share_revoke：按 source=share:<id> 级联删 allowlist 条目。
#[tauri::command]
pub async fn llm_share_share_revoke(
    store: State<'_, LlmShareStore>,
    share_id: String,
) -> Result<ShareRevokeReport, String> {
    flows_share::share_revoke(&store, &share_id)
}

/// llm_share_share_redeem：借方兑换；结构化拒绝码原样透出（业务结果非 Err）。
#[tauri::command]
pub async fn llm_share_share_redeem(
    store: State<'_, LlmShareStore>,
    state: State<'_, AppState>,
    link: String,
) -> Result<RedeemOutcome, String> {
    flows_share::share_redeem(&store, &state.config_get(), link).await
}

/// llm_share_serve_status：出借方常驻 serve 装配状态（assembled:false 常态非故障）。
#[tauri::command]
pub async fn llm_share_serve_status(state: State<'_, AppState>) -> Result<LlmServeStatus, String> {
    Ok(state.llm_serve_status().await)
}
