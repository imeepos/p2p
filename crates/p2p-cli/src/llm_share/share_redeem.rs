//! share_redeem 借方编排（llm-share-link 设计 §5.4，W3）：解析链接 → 一次性节点
//! 拨号（链接 addr 登记优先，缺省 rendezvous 查号）→ /llm-share/redeem/1 请求帧
//! 兑换 → 成功后经既有 offer 拉取（borrow_dial::fetch_offer）取回快照。
//! 结构化拒绝码照契约 §16.6 原样透出（业务结果非 Err）；scheme/peer/token
//! 缺失或非法 = 参数错误显式 Err。兑换已激活后再失败可安全重试：同 peer
//! 二次兑换在出借方幂等成功，不重复消耗激活次数。

use std::collections::BTreeMap;
use std::time::Duration;

use llm_share_link::link::{parse_link, LinkError, ShareLink};
use llm_share_link::redeem::{
    RedeemCode, RedeemRequest, RedeemResponse, PROTOCOL_ID as REDEEM_PROTOCOL,
};
use llm_share_offer::OfferBook;
use p2p::{Node, PeerId};
use serde::Serialize;

use super::borrow_dial::{self, NodeShutdown};
use super::{now_secs, validate_peer_id};

/// 兑换单步（拨号/请求/应答）超时秒：轻量一问一答，30s 宽松护栏。
pub const REDEEM_TIMEOUT_SECS: u64 = 30;

/// rendezvous 查号等待秒（对齐 borrow 缺省）。
pub const REDEEM_DISCOVER_SECS: u64 = 10;

/// share_redeem 入参（apps 层装配：链接原文 + 节点环境）。
pub struct RedeemParams {
    /// dsh-llm-share:// 链接原文。
    pub link: String,
    /// rendezvous bootstrap 地址（链接无 addr 时查号用）。
    pub bootstrap: Vec<String>,
    /// 节点数据目录（借方身份种子同根，须与出借方绑定一致）。
    pub node_dir: String,
    pub timeout_secs: u64,
    pub discover_secs: u64,
}

/// 出借方 offer 快照摘要（契约 §16.6 LlmShareRedeemResult.offer）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedeemOfferSummary {
    pub peer: String,
    pub models: Vec<String>,
    pub spare: BTreeMap<String, u64>,
    pub period_ends: String,
}

/// 兑换结果：rejected 是业务结果非命令 Err（拒绝码 wire kebab-case 原样）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedeemOutcome {
    pub status: RedeemStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offer: Option<RedeemOfferSummary>,
    /// 链接 sid 提示值（权威 shareId 在出借方台账；链接未带则为 None）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub share_id: Option<String>,
    /// 出借方 PeerId（offer 快照 peer，验签后可信）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RedeemStatus {
    Redeemed,
    Rejected,
}

/// 主流程：解析（参数错误显式 Err）→ 拨号 → 兑换帧 → 拒绝码透出 / offer 快照。
pub async fn run(params: &RedeemParams) -> Result<RedeemOutcome, String> {
    let link = parse_link(&params.link).map_err(link_param_error)?;
    let lender = parse_peer(&link.peer)?;
    let node = borrow_dial::build_node(&params.node_dir, &params.bootstrap).await?;
    let guard = NodeShutdown::new(node);
    connect_lender(guard.node(), lender, &link, params).await?;
    match request_redeem(guard.node(), lender, &link, params).await? {
        RedeemResponse {
            ok: true,
            code: None,
        } => redeemed_outcome(guard.node(), lender, &link).await,
        RedeemResponse {
            ok: false,
            code: Some(code),
        } => Ok(RedeemOutcome {
            status: RedeemStatus::Rejected,
            code: Some(code_str(code)),
            offer: None,
            share_id: link.sid.clone(),
            owner: None,
        }),
        RedeemResponse {
            ok: false,
            code: None,
        } => Err("REDEEM-FAIL: 兑换应答缺拒绝码（协议违例）".into()),
        RedeemResponse {
            ok: true,
            code: Some(_),
        } => Err("REDEEM-FAIL: 兑换应答成功帧不得携带拒绝码（协议违例）".into()),
    }
}

/// 建连：链接 addr 逐条登记后直连；无 addr 走 rendezvous 查号（bootstrap 注入）。
async fn connect_lender(
    node: &Node,
    lender: PeerId,
    link: &ShareLink,
    params: &RedeemParams,
) -> Result<(), String> {
    if link.addrs.is_empty() {
        let addr = borrow_dial::discover(node, &link.peer, params.discover_secs).await?;
        register(node, lender, &addr)?;
    } else {
        for addr in &link.addrs {
            register(node, lender, addr)?;
        }
    }
    tokio::time::timeout(
        Duration::from_secs(params.timeout_secs),
        node.connect(lender),
    )
    .await
    .map_err(|_| "REDEEM-FAIL: 出借方建连超时".to_owned())?
    .map_err(|e| format!("REDEEM-FAIL: {e}"))
}

fn register(node: &Node, lender: PeerId, addr: &str) -> Result<(), String> {
    node.add_peer_address(lender, addr)
        .map_err(|e| format!("REDEEM-FAIL: 链接地址非法（{addr}）: {e}"))
}

async fn request_redeem(
    node: &Node,
    lender: PeerId,
    link: &ShareLink,
    params: &RedeemParams,
) -> Result<RedeemResponse, String> {
    let id = p2p::ProtocolId::new(REDEEM_PROTOCOL)
        .map_err(|e| format!("REDEEM-FAIL: 协议 ID 非法: {e}"))?;
    let payload = serde_json::to_vec(&RedeemRequest {
        token: link.token.clone(),
    })
    .map_err(|e| format!("REDEEM-FAIL: 请求帧编码失败: {e}"))?;
    let raw = node
        .request(
            lender,
            id,
            payload,
            Duration::from_secs(params.timeout_secs),
        )
        .await
        .map_err(|e| format!("REDEEM-FAIL: 兑换请求失败: {e}"))?;
    serde_json::from_slice(&raw).map_err(|e| format!("REDEEM-FAIL: 兑换应答解析失败: {e}"))
}

/// 兑换成功：经既有 offer 拉取取回快照并验签（拉取失败显式 Err——兑换已绑定本
/// peer，重试在出借方幂等成功，不重复消耗激活）。
async fn redeemed_outcome(
    node: &Node,
    lender: PeerId,
    link: &ShareLink,
) -> Result<RedeemOutcome, String> {
    let signed = borrow_dial::fetch_offer(node, lender).await?;
    let mut book = OfferBook::new();
    book.insert(signed.clone(), now_secs())
        .map_err(|e| format!("OFFER-VERIFY-FAIL: {e}"))?;
    Ok(RedeemOutcome {
        status: RedeemStatus::Redeemed,
        code: None,
        offer: Some(RedeemOfferSummary {
            peer: signed.offer.peer.clone(),
            models: signed.offer.models.clone(),
            spare: signed.offer.spare.clone(),
            period_ends: signed.offer.period_ends.clone(),
        }),
        share_id: link.sid.clone(),
        owner: Some(signed.offer.peer),
    })
}

/// 拒绝码 wire 原文（RedeemCode serde kebab-case）。
pub fn code_str(code: RedeemCode) -> String {
    let json = serde_json::to_string(&code).unwrap_or_default();
    json.trim_matches('"').to_owned()
}

/// 链接形态错误 → 参数错误显式 Err（契约 §16.6：scheme/peer/token 缺失非业务码）。
fn link_param_error(e: LinkError) -> String {
    match e {
        LinkError::BadScheme(actual) => {
            format!(
                "链接 scheme 不符（期望 {}，实际 {actual}）",
                llm_share_link::SCHEME
            )
        }
        LinkError::MissingPeer => "链接缺少 peer 参数".into(),
        LinkError::MissingToken => "链接缺少 token 参数".into(),
        LinkError::BadToken(token) => format!("链接 token 非法（应恰 32 位小写 hex）：{token}"),
    }
}

fn parse_peer(peer_id: &str) -> Result<PeerId, String> {
    validate_peer_id(peer_id)?;
    let raw = bs58::decode(peer_id)
        .into_vec()
        .map_err(|_| format!("PeerId 非法（不是合法 base58）：{peer_id}"))?;
    let bytes: [u8; 32] = raw
        .try_into()
        .map_err(|_| format!("PeerId 非法（解码后应恰 32 字节）：{peer_id}"))?;
    Ok(PeerId::from_bytes(bytes))
}

#[cfg(test)]
mod tests;
