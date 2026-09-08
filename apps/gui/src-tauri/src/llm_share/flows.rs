//! 契约 §16.1 九命令主流程（plain fns，tests 直调冒烟；commands.rs 只做 State 抽取
//! 委托）。写路径全部经 p2p-cli llm_share 逻辑层（与 CLI 同一条 tmp+rename 原子写），
//! 查询命令零写入（§16.2.4）；结构化失败一律可读中文 Err。

use p2p_cli::llm_share::{
    allowlist, borrow, borrow::BorrowParams, ledger, ledger::LedgerFilters, offer, receipt,
};
use uuid::Uuid;

use super::inputs::{LlmBorrowRequest, LlmLedgerFilter, LlmOfferPublishInput};
use super::serve::gate::AllowlistGate;
use super::views::{
    LlmAllowlistView, LlmBalanceGroup, LlmBorrowReport, LlmLedgerEntry, LlmOfferView,
    LlmReceiptVerifyResult,
};
use super::LlmShareStore;
use crate::types::GuiConfig;

/// borrow 代理调用全程超时（对齐 CLI 缺省 90s，契约 §16.1 不外露）。
const BORROW_TIMEOUT_SECS: u64 = 90;

/// rendezvous 查号等待秒（对齐 CLI 缺省 10s）。
const BORROW_DISCOVER_SECS: u64 = 10;

/// llm_share_offer_publish：IPC 必填集预检 → 组装签名发布 → 视图（status 缺省）。
pub fn offer_publish(
    store: &LlmShareStore,
    cfg: &GuiConfig,
    input: &LlmOfferPublishInput,
) -> Result<LlmOfferView, String> {
    validate_publish_input(input)?;
    let params = offer::OfferParams {
        models: input.models.clone(),
        spare: format_pairs(&input.spare),
        period_ends: input.period_ends.clone(),
        max_per_req: format_pairs(&input.max_per_req),
        rpm: input.rpm,
        concurrency: input.concurrency,
        ttl_secs: input.ttl_secs,
        retention: input.retention.clone(),
    };
    let report = offer::publish(
        &store.seed_path(cfg),
        &store.data_dir(),
        &params,
        now_secs(),
    )?;
    Ok(LlmOfferView::from(&report))
}

/// llm_share_offer_show：读信封验签定五态（信封损坏显式 Err，不伪造状态）。
pub fn offer_show(store: &LlmShareStore) -> Result<LlmOfferView, String> {
    let report = offer::show(&store.data_dir(), now_secs())?;
    LlmOfferView::from_show(report)
}

/// llm_share_allow_list：白名单清单（缺失视为空表，默认拒绝语义）。
pub fn allow_list(store: &LlmShareStore) -> Result<LlmAllowlistView, String> {
    allowlist::list(&store.data_dir()).map(LlmAllowlistView::from)
}

/// llm_share_allow：upsert（models 缺省=不限模型），返回变更后视图。
/// source/expires_at 透传（§16.6 v13 allow 命令适配；缺省 None 语义不变，
/// source 系分享兑换注入口，手工 allow 恒为 None）。serve 在装配时经
/// AllowlistGate 写入（内存+磁盘同源，admit 即时生效），否则纯文件写。
pub fn allow(
    store: &LlmShareStore,
    gate: Option<&AllowlistGate>,
    peer_id: &str,
    models: &[String],
    note: Option<&str>,
    source: Option<&str>,
    expires_at: Option<u64>,
) -> Result<LlmAllowlistView, String> {
    let granted_at = p2p_cli::llm_share::rfc3339_now();
    match gate {
        Some(gate) => {
            gate.allow(peer_id, models, note, source, expires_at, &granted_at)?;
        }
        None => {
            allowlist::allow(
                &store.data_dir(),
                peer_id,
                models,
                note,
                source,
                expires_at,
                &granted_at,
            )?;
        }
    }
    allow_list(store)
}

/// llm_share_deny：移出白名单；不存在条目显式 Err（默认拒绝语义非故障，
/// §16.1）。serve 在装配时经 AllowlistGate（内存+磁盘同源）。
pub fn deny(
    store: &LlmShareStore,
    gate: Option<&AllowlistGate>,
    peer_id: &str,
) -> Result<LlmAllowlistView, String> {
    match gate {
        Some(gate) => gate.deny(peer_id)?,
        None => allowlist::deny(&store.data_dir(), peer_id)?,
    };
    allow_list(store)
}

/// llm_share_ledger_list：过滤后流水明细（append-only 存储序）。
pub fn ledger_list(
    store: &LlmShareStore,
    filter: &LlmLedgerFilter,
) -> Result<Vec<LlmLedgerEntry>, String> {
    let filters = LedgerFilters {
        lender: filter.lender.as_deref(),
        borrower: filter.borrower.as_deref(),
        period: filter.period.as_deref(),
    };
    let report = ledger::list(&store.data_dir(), filters)?;
    Ok(report
        .entries
        .into_iter()
        .map(LlmLedgerEntry::from)
        .collect())
}

/// llm_share_ledger_balance：本机视角净差（无参取本机身份；未参与返回空行）。
pub fn ledger_balance(
    store: &LlmShareStore,
    cfg: &GuiConfig,
) -> Result<Vec<LlmBalanceGroup>, String> {
    let self_peer = store.local_peer_id(cfg)?;
    let report = ledger::balance(&store.data_dir(), &self_peer, None)?;
    Ok(report.rows.into_iter().map(LlmBalanceGroup::from).collect())
}

/// llm_share_receipt_verify：reqId 定位单笔收据文件离线验签；lenderPubkey 缺省取
/// 本机身份（出借方自验，ai-guide 口径），借方场景显式传入。
pub fn receipt_verify(
    store: &LlmShareStore,
    cfg: &GuiConfig,
    req_id: &str,
    lender_pubkey: Option<&str>,
) -> Result<LlmReceiptVerifyResult, String> {
    let file = store.receipt_file(req_id)?;
    let pubkey = match lender_pubkey {
        Some(pk) if !pk.trim().is_empty() => pk.to_owned(),
        _ => store.local_pubkey_base58(cfg).map_err(|e| {
            format!("{e}；借方场景可显式传入出借方公钥 lenderPubkey 或从账本条目取得")
        })?,
    };
    receipt::verify_file(&file, &pubkey).map(LlmReceiptVerifyResult::from)
}

/// llm_share_borrow：一次性拨号借方调用（p2p-cli borrow::run 编排：验签选路 →
/// 三闸预检 → 代理流式 → 收据复验 → 幂等入账）；rejected 是业务结果照常返回报告。
pub async fn borrow(
    store: &LlmShareStore,
    cfg: &GuiConfig,
    req: LlmBorrowRequest,
) -> Result<LlmBorrowReport, String> {
    let plan = normalize_borrow_request(req)?;
    let params = BorrowParams {
        lender: plan.target_peer,
        model: plan.model,
        prompt: None,
        messages_json: Some(messages_payload(&plan.messages)),
        max_tokens: Some(plan.max_tokens),
        addr: None,
        bootstrap: cfg.bootstrap.clone(),
        node_dir: store.node_dir(cfg).to_string_lossy().into_owned(),
        ledger_dir: store.data_dir(),
        timeout_secs: BORROW_TIMEOUT_SECS,
        discover_secs: BORROW_DISCOVER_SECS,
        req_id: Some(plan.req_id),
    };
    borrow::run(&params).await.map(LlmBorrowReport::from)
}

/// 表单「消息」向 wire 契约的适配（§16.2-6）：表单承诺纯文本，OpenAI 数组
/// JSON 也收——数组原样透传，其余（纯文本/非数组 JSON）一律包装为单条用户
/// 消息，对齐 CLI --prompt 语义；GUI 侧不再出现「--messages 不是合法 JSON」。
pub(crate) fn messages_payload(text: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(value) if value.is_array() => text.to_owned(),
        _ => {
            let content = serde_json::Value::String(text.to_owned());
            let arr = serde_json::Value::Array(vec![serde_json::json!({
                "role": "user",
                "content": content,
            })]);
            serde_json::to_string(&arr).unwrap_or_default()
        }
    }
}

/// borrow 请求规约化结果（tests 直调覆盖必填缺省报错路径）。
#[derive(Debug)]
pub(crate) struct BorrowPlan {
    pub target_peer: String,
    pub model: Option<String>,
    pub messages: String,
    pub max_tokens: u64,
    pub req_id: String,
}

/// §16.1/§16.2.6 IPC 层显性校验：maxTokens 与 targetPeer 无缺省路径，缺省显式报错；
/// reqId 缺省生成 UUID v4（重试复用同值防双记账）。
pub(crate) fn normalize_borrow_request(req: LlmBorrowRequest) -> Result<BorrowPlan, String> {
    let target_peer = non_empty(req.target_peer, "targetPeer 必填：GUI 无缺省出借方路径")?;
    let max_tokens = req
        .max_tokens
        .ok_or("maxTokens 必填：borrow 是真实成本动作，须显式单请求上限（契约 §16.2.6）")?;
    let messages = non_empty(req.messages, "messages 必填：OpenAI messages 数组 JSON")?;
    let req_id = match non_empty(req.req_id, "") {
        Ok(id) => id,
        Err(_) => Uuid::new_v4().to_string(),
    };
    Ok(BorrowPlan {
        target_peer,
        model: req.model,
        messages,
        max_tokens,
        req_id,
    })
}

fn non_empty(value: Option<String>, message: &str) -> Result<String, String> {
    value
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| message.to_owned())
}

fn validate_publish_input(input: &LlmOfferPublishInput) -> Result<(), String> {
    if input.models.is_empty() {
        return Err("models 必填：至少声明一个模型".to_owned());
    }
    if input.spare.is_empty() {
        return Err("spare 必填：须为每个声明模型给出正闲量（model=N 且 N>0）".to_owned());
    }
    Ok(())
}

/// BTreeMap → "model=N" 键值原文（BTreeMap 序稳定且无重复键，crate 解析兜底格式）。
fn format_pairs(map: &std::collections::BTreeMap<String, u64>) -> Vec<String> {
    map.iter()
        .map(|(model, n)| format!("{model}={n}"))
        .collect()
}

fn now_secs() -> u64 {
    p2p_cli::llm_share::now_secs()
}
