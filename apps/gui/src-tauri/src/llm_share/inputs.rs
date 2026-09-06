//! 契约 §16.1 IPC 入参形状（camelCase；必填缺省报错在 flows 规约化层显式给出）。

use std::collections::BTreeMap;

use serde::Deserialize;

/// llm_share_offer_publish 入参（§16.1 必填集：models ≥1、spare 覆盖全部 model 且
/// N>0、periodEnds 日期；rpm/concurrency/ttl/retention 缺省对齐 CLI 默认）。
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmOfferPublishInput {
    pub models: Vec<String>,
    pub spare: BTreeMap<String, u64>,
    pub period_ends: String,
    #[serde(default)]
    pub max_per_req: BTreeMap<String, u64>,
    #[serde(default = "default_rpm")]
    pub rpm: u32,
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    #[serde(default = "default_ttl_secs")]
    pub ttl_secs: u64,
    #[serde(default)]
    pub retention: Option<String>,
}

fn default_rpm() -> u32 {
    10
}
fn default_concurrency() -> u32 {
    2
}
fn default_ttl_secs() -> u64 {
    3600
}

/// llm_share_ledger_list 过滤器（§16.1 filter{lender?, borrower?, period?}）。
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmLedgerFilter {
    pub lender: Option<String>,
    pub borrower: Option<String>,
    pub period: Option<String>,
}

/// llm_share_borrow 入参（§16.1 req：maxTokens/targetPeer 必填由 IPC 层显式报错；
/// reqId 缺省 IPC 层生成 UUID v4；messages 为 OpenAI messages 数组 JSON）。
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmBorrowRequest {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub messages: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub target_peer: Option<String>,
    #[serde(default)]
    pub req_id: Option<String>,
}
