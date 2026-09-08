//! 契约 §16.6 v13 视图壳：provider 清单/移除包络。其余返回类型直接复用
//! p2p-cli 共享事实源的 serde 形状（ProviderView/ShareCreateReport/
//! ShareListReport/ShareRevokeReport/RedeemOutcome，chat.rs 先例），
//! apiKey 只出掩码、台账永不含 token 原文与哈希。

use p2p_cli::llm_share::provider::ProviderView;
use serde::Serialize;

/// provider 清单包络（契约：{ providers: LlmProviderView[] }）。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderListView {
    pub providers: Vec<ProviderView>,
}

/// provider 移除包络（契约：{ removed: true }；不存在=显式 Err 不到此）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderRemoveView {
    pub removed: bool,
    pub provider_id: String,
}
