//! 工单票据：铸造（mint）与校验（verify，helper 受理前置）。
//!
//! 契约（remote-support-plan.md §3.3）：载荷 canonical JSON
//! {"ticket_id","helper_peer","bridge_peer","scope","iat","exp"}（时间 Unix 秒），
//! 编码 base64url(payload) + "." + base64url(ed25519 签名)。helper 侧全过才
//! 受理，顺序：签名验真（平台公钥参数注入）/ exp 未过 / scope 枚举合法 /
//! 入流对端 PeerId == bridge_peer / 工单存活表查重（一次性）。
//!
//! 只消费 p2p-identity 冻结契约（[Keypair]/[PeerId]），不修改其接口。

use p2p_identity::{Keypair, PeerId};
use serde::{Deserialize, Serialize};

pub use crate::ticket_ledger::TicketLedger;

pub const SCOPE_DIAG: &str = "diag";
pub const SCOPE_FIX: &str = "fix";
pub const SCOPES: [&str; 2] = [SCOPE_DIAG, SCOPE_FIX];

/// 票据载荷（字段顺序即 canonical JSON 顺序，mint/verify 同源）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TicketPayload {
    pub ticket_id: String,
    pub helper_peer: String,
    pub bridge_peer: String,
    pub scope: String,
    pub iat: u64,
    pub exp: u64,
}

/// 票据校验失败原因（留痕用，helper 侧拒绝即带此因）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TicketError {
    /// 结构/编码/载荷解析失败（base64url、JSON、PeerId）。
    Malformed(String),
    /// 签名验真失败（平台公钥不匹配或载荷被篡改）。
    BadSignature,
    /// exp 已过（过期票据一票否决）。
    Expired,
    /// scope 不在 diag/fix 枚举内。
    BadScope(String),
    /// 入流对端与 ticket.bridge_peer 不一致。
    PeerMismatch { bridge_peer: String },
    /// 工单存活表已登记（一次性票据重复使用）。
    AlreadyUsed,
    /// 载荷序列化失败（铸造侧）。
    Encode(String),
}

impl std::fmt::Display for TicketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TicketError::Malformed(why) => write!(f, "malformed ticket: {why}"),
            TicketError::BadSignature => f.write_str("bad signature"),
            TicketError::Expired => f.write_str("ticket expired"),
            TicketError::BadScope(scope) => write!(f, "bad scope: {scope}"),
            TicketError::PeerMismatch { bridge_peer } => {
                write!(
                    f,
                    "inbound peer mismatch (ticket bridge_peer={bridge_peer})"
                )
            }
            TicketError::AlreadyUsed => f.write_str("ticket already used"),
            TicketError::Encode(why) => write!(f, "ticket encode failed: {why}"),
        }
    }
}

impl std::error::Error for TicketError {}

/// base64url 编码（无填充；§3.3 契约编码，mint 与 verify 同侧实现）。
fn b64url_encode(bytes: &[u8]) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.encode(bytes)
}

fn b64url_decode(s: &str) -> Result<Vec<u8>, TicketError> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|e| TicketError::Malformed(format!("base64url: {e}")))
}

/// 铸造票据：平台密钥对 canonical payload 签名（P0b 经 mint-ticket 子命令，
/// 生产由调度签发属 P1）。scope 非法直接拒绝，不产出票据。
pub fn mint(
    key: &Keypair,
    ticket_id: &str,
    helper_peer: &str,
    bridge_peer: &str,
    scope: &str,
    ttl_secs: u64,
    now_unix: u64,
) -> Result<String, TicketError> {
    if !SCOPES.contains(&scope) {
        return Err(TicketError::BadScope(scope.to_string()));
    }
    let payload = TicketPayload {
        ticket_id: ticket_id.to_string(),
        helper_peer: helper_peer.to_string(),
        bridge_peer: bridge_peer.to_string(),
        scope: scope.to_string(),
        iat: now_unix,
        exp: now_unix.saturating_add(ttl_secs),
    };
    let body = serde_json::to_string(&payload).map_err(|e| TicketError::Encode(e.to_string()))?;
    let sig = key.sign(body.as_bytes());
    Ok(format!(
        "{}.{}",
        b64url_encode(body.as_bytes()),
        b64url_encode(&sig)
    ))
}

/// 校验上下文：平台公钥（参数注入）+ 工单存活表（一次性）。
#[derive(Clone)]
pub struct TicketVerifier {
    platform_pubkey: [u8; 32],
    ledger: TicketLedger,
}

impl TicketVerifier {
    pub fn new(platform_pubkey: [u8; 32], ledger: TicketLedger) -> Self {
        Self {
            platform_pubkey,
            ledger,
        }
    }

    /// 全项校验（§3.3 顺序）：签名 -> exp -> scope -> 对端 -> 查重；
    /// 全过才登记一次性票据并返回载荷。任一失败留原因（[TicketError]）。
    pub fn verify(
        &self,
        ticket: &str,
        inbound_peer: &PeerId,
        now_unix: u64,
    ) -> Result<TicketPayload, TicketError> {
        let (body_b64, sig_b64) = ticket
            .split_once('.')
            .ok_or_else(|| TicketError::Malformed("missing '.' separator".to_string()))?;
        // 签名消息 = base64url 内容原始字节（mint 即对 canonical JSON 字节签名）。
        let body = b64url_decode(body_b64)?;
        let sig: [u8; 64] = b64url_decode(sig_b64)?
            .try_into()
            .map_err(|_| TicketError::Malformed("signature not 64 bytes".to_string()))?;
        if !Keypair::verify(&self.platform_pubkey, &body, &sig) {
            return Err(TicketError::BadSignature);
        }
        let payload: TicketPayload = serde_json::from_slice(&body)
            .map_err(|e| TicketError::Malformed(format!("payload json: {e}")))?;
        if now_unix >= payload.exp {
            return Err(TicketError::Expired);
        }
        if !SCOPES.contains(&payload.scope.as_str()) {
            return Err(TicketError::BadScope(payload.scope));
        }
        let bridge_peer = parse_peer_id(&payload.bridge_peer)?;
        if *inbound_peer != bridge_peer {
            return Err(TicketError::PeerMismatch {
                bridge_peer: payload.bridge_peer.clone(),
            });
        }
        self.ledger
            .claim(&payload.ticket_id, payload.exp, now_unix)?;
        Ok(payload)
    }
}

/// base58 -> PeerId；长度/编码错误显式上抛（与 CLI 解析同规则）。
pub fn parse_peer_id(s: &str) -> Result<PeerId, TicketError> {
    let bytes = bs58::decode(s)
        .into_vec()
        .map_err(|e| TicketError::Malformed(format!("bridge peer base58: {e}")))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| TicketError::Malformed("bridge peer not 32 bytes".to_string()))?;
    Ok(PeerId::from_bytes(arr))
}
