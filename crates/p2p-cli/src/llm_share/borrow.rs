//! borrow 主流程（F11/PR6，idle-token-sharing-plan §4 借方侧）：
//! 连接出借方 → 拉取声明并验签选路（TTL/模型过滤走产品纯函数 select_offers）→
//! 预检（出借方三闸前置裁决：NotAllowlisted 等结构化拒绝在进入上游前透出，
//! 不产生流水、不入账、不重试）→ /llm-share/proxy/1 流式调用 → 收据 Ed25519
//! 验签（client 强制 + 落盘前复验）→ 借方账本入账（§5.1 wire 形态）。
//! 拨号装配进程内自建（facade Node，见 borrow_dial），不改 daemon/serve 常驻行为。

use std::sync::Arc;
use std::time::Duration;

use llm_share_ledger::{Receipt, WINDOW_ESTIMATED_SECS, WINDOW_SECS};
use llm_share_offer::{select_offers, OfferBook, SignedOffer};
use llm_share_proxy::{ProxyClient, ProxyEvent, ProxyRequest};
use p2p::Node;
use p2p_identity::PeerId;
use serde_json::Value;

use super::borrow_report::{self, BorrowReport};
use super::ledger;
use super::{borrow_dial, file_path, now_secs, validate_peer_id, write_json_atomic};

/// 输出上限缺省：出借方声明未设 max_per_req 时生效（预授权仍由出借方实校）。
const DEFAULT_MAX_TOKENS: u64 = 256;

/// borrow 参数（apps/cli clap 层装配，本层校验与编排）。
pub struct BorrowParams {
    /// 出借方 PeerId（base58）。
    pub lender: String,
    /// 模型名；None = 声明内唯一模型，多模型显式报错列出可选项。
    pub model: Option<String>,
    /// 单条用户消息（OpenAI messages 等价语义）。
    pub prompt: Option<String>,
    /// messages 数组 JSON 原文（OpenAI chat completions 等价）。
    pub messages_json: Option<String>,
    /// 单请求 max_tokens；None = 出借方声明上限，未声明则 DEFAULT_MAX_TOKENS。
    pub max_tokens: Option<u64>,
    /// 出借方直连地址；None 走 rendezvous 查号（bootstrap 由装配方注入）。
    pub addr: Option<String>,
    pub bootstrap: Vec<String>,
    /// 节点数据目录（key.seed 同根：身份须与出借方 allowlist 登记一致）。
    pub node_dir: String,
    /// 流水目录（<data-dir>/llm-share/ledger.json）。
    pub ledger_dir: String,
    pub timeout_secs: u64,
    pub discover_secs: u64,
    /// 幂等键；None 生成 UUID v4（重试复用同值防双记账，MVP A4）。
    pub req_id: Option<String>,
}

/// 进程退出前确保节点关停（错误路径同样断连，不残留监听）。
struct NodeShutdown(Arc<Node>);
impl Drop for NodeShutdown {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}

impl NodeShutdown {
    fn new(node: Arc<Node>) -> Self {
        Self(node)
    }

    fn node(&self) -> &Node {
        &self.0
    }

    fn raw(&self) -> Arc<Node> {
        self.0.clone()
    }
}

/// borrow 主流程。结构化拒绝（rejected）不视为运行错误：报告照常返回，
/// 由命令面决定退出码（对齐 receipt verify 的「报告照常输出 + 显式失败」）。
pub async fn run(params: &BorrowParams) -> Result<BorrowReport, String> {
    validate_peer_id(&params.lender)?;
    let messages = build_messages(params)?;
    let lender = parse_lender(&params.lender)?;
    let guard = NodeShutdown::new(borrow_dial::build_node(params).await?);
    borrow_dial::connect_lender(guard.node(), lender, params).await?;
    let signed = borrow_dial::fetch_offer(guard.node(), lender).await?;
    let model = resolve_model(params.model.as_deref(), &signed)?;
    let now = now_secs();
    let mut book = OfferBook::new();
    book.insert(signed.clone(), now)
        .map_err(|e| format!("OFFER-VERIFY-FAIL: {e}"))?;
    let candidate = select_offers(book.live(now), &model, now)
        .into_iter()
        .find(|c| c.peer == signed.offer.peer)
        .ok_or_else(|| {
            format!("OFFER-ROUTE-FAIL: 模型 {model} 无有效候选（TTL 过期或声明零闲量）")
        })?;
    let max_tokens = resolve_max_tokens(params.max_tokens, candidate.max_per_req)?;
    let req = build_request(params, &model, max_tokens, messages)?;
    let client = ProxyClient::new(borrow_dial::NodeFactory { node: guard.raw() });
    let events = client
        .call(
            lender,
            &req,
            signed.pubkey,
            Duration::from_secs(params.timeout_secs),
        )
        .await
        .map_err(|e| format!("CALL-FAIL: {e}"))?;
    let sse: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            ProxyEvent::Sse(d) => Some(d.clone()),
            _ => None,
        })
        .collect();
    let sse_frames = sse.len();
    match events.last() {
        Some(ProxyEvent::Finished {
            receipt,
            stream_broken,
        }) => finish(
            params,
            receipt,
            *stream_broken,
            &signed.pubkey,
            sse_frames,
            sse,
        ),
        Some(ProxyEvent::Rejected { code, message, .. }) => Ok(borrow_report::rejected_report(
            params, &req, code, message, sse_frames, sse,
        )),
        _ => Err("CALL-FAIL: 空事件序列（协议违例）".into()),
    }
}

/// 终结路径：落盘前复验收据签名（断流 estimated 收据同样必须过验签），
/// 再经 ledger::record 幂等入账。
fn finish(
    params: &BorrowParams,
    receipt: &Receipt,
    stream_broken: bool,
    lender_pubkey: &[u8; 32],
    sse_frames: usize,
    sse: Vec<String>,
) -> Result<BorrowReport, String> {
    receipt
        .verify(lender_pubkey)
        .map_err(|e| format!("RECEIPT-VERIFY-FAIL: {e}"))?;
    let record = ledger::record(&params.ledger_dir, receipt)?;
    let receipt_file = file_path(
        &params.ledger_dir,
        &format!("receipt-{}.json", receipt.req_id),
    );
    write_json_atomic(&receipt_file, receipt, "收据")?;
    Ok(BorrowReport {
        status: if stream_broken {
            "stream_broken"
        } else {
            "done"
        },
        lender: receipt.lender.clone(),
        model: receipt.model.clone(),
        req_id: receipt.req_id.clone(),
        period: receipt.period.clone(),
        code: None,
        message: None,
        input: receipt.usage.input,
        output: receipt.usage.output,
        estimated: receipt.estimated,
        dispute_window_secs: if receipt.estimated {
            WINDOW_ESTIMATED_SECS
        } else {
            WINDOW_SECS
        },
        sse_frames,
        ledger_file: record.file,
        receipt_file: receipt_file.display().to_string(),
        appended: record.appended,
        sse,
    })
}

/// 对话内容：--prompt 单条用户消息，或 --messages 数组 JSON（OpenAI 等价语义），
/// 二选一；两者皆缺显式报错。
fn build_messages(params: &BorrowParams) -> Result<Value, String> {
    match (params.prompt.as_deref(), params.messages_json.as_deref()) {
        (Some(_), Some(_)) => Err("--prompt 与 --messages 二选一".into()),
        (Some(text), None) => Ok(serde_json::json!([{ "role": "user", "content": text }])),
        (None, Some(json)) => parse_messages(json),
        (None, None) => Err("缺少对话内容：--prompt 或 --messages 必填其一".into()),
    }
}

fn parse_messages(json: &str) -> Result<Value, String> {
    let value: Value =
        serde_json::from_str(json).map_err(|e| format!("--messages 不是合法 JSON: {e}"))?;
    let Some(items) = value.as_array() else {
        return Err("--messages 必须是 OpenAI messages 数组".into());
    };
    if items.is_empty() {
        return Err("--messages 不能为空数组".into());
    }
    for item in items {
        let valid = item
            .get("role")
            .and_then(Value::as_str)
            .is_some_and(|r| !r.is_empty())
            && item.get("content").is_some();
        if !valid {
            return Err("--messages 元素须含 role 与 content 字段".into());
        }
    }
    Ok(value)
}

/// 模型裁决：显式指定必须在出借方声明内；缺省取声明内唯一模型，多模型列清单报错。
fn resolve_model(want: Option<&str>, signed: &SignedOffer) -> Result<String, String> {
    let models = &signed.offer.models;
    match want {
        Some(name) if models.iter().any(|m| m == name) => Ok(name.to_owned()),
        Some(name) => Err(format!(
            "MODEL-NOT-OFFERED: 出借方不供模型 {name}（声明内可选：{}）",
            models.join(", ")
        )),
        None if models.len() == 1 => Ok(models[0].clone()),
        None => Err(format!(
            "MODEL-REQUIRED: 出借方声明多个模型，--model 必填其一：{}",
            models.join(", ")
        )),
    }
}

/// max_tokens 裁决：显式值超出声明上限显式报错（不静默截断）；
/// 缺省取声明上限，未声明回落 DEFAULT_MAX_TOKENS。
fn resolve_max_tokens(want: Option<u64>, declared: Option<u64>) -> Result<u64, String> {
    match (want, declared) {
        (Some(v), Some(cap)) if v > cap => {
            Err(format!("MAX-TOKENS-EXCEEDED: {v} 超出出借方声明上限 {cap}"))
        }
        (Some(v), _) => Ok(v),
        (None, cap) => Ok(cap.unwrap_or(DEFAULT_MAX_TOKENS)),
    }
}

fn build_request(
    params: &BorrowParams,
    model: &str,
    max_tokens: u64,
    messages: Value,
) -> Result<ProxyRequest, String> {
    let req_id = params
        .req_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    Ok(ProxyRequest {
        req_id,
        wire_bytes: 0,
        body: serde_json::json!({
            "model": model,
            "max_tokens": max_tokens,
            "stream": true,
            "messages": messages
        }),
        model: model.to_owned(),
        max_tokens,
    })
}

/// base58 → PeerId（validate_peer_id 已挡非法，此处只做类型转换）。
fn parse_lender(peer_id: &str) -> Result<PeerId, String> {
    let raw = bs58::decode(peer_id)
        .into_vec()
        .map_err(|_| format!("PeerId 非法（不是合法 base58）：{peer_id}"))?;
    let bytes: [u8; 32] = raw
        .try_into()
        .map_err(|_| format!("PeerId 非法（解码后应恰 32 字节）：{peer_id}"))?;
    Ok(PeerId::from_bytes(bytes))
}
