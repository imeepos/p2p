//! 消息模型与校验（design §5；契约 gui-contract.md §12.3）。
//!
//! 序列化形状逐字对齐契约（camelCase，Option 序列化 null）；校验失败一律可读中文 Err 禁止静默降级。

use std::time::{SystemTime, UNIX_EPOCH};

use p2p_identity::PeerId;
use serde::{Deserialize, Serialize};

/// 单条消息（含附件原始字节）上限，与 chunked.rs MAX_MESSAGE_SIZE 一致。
pub const MAX_MESSAGE_SIZE: u64 = 64 << 20;

/// 文本上限（trim 后字符数）。
pub const MAX_TEXT_CHARS: usize = 2000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sender {
    Me,
    Them,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatStatus {
    Pending,
    Sent,
    Delivered,
    Failed,
}

/// 附件元数据；path 仅本端落盘路径，不跨网。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMediaMeta {
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub path: Option<String>,
}

/// 消息信封（本地与 JSONL 落盘共用形状，字段与契约逐字一致）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatEnvelope {
    pub id: String,
    pub peer: String,
    pub sender: Sender,
    pub kind: ChatKind,
    #[serde(rename = "tsMs")]
    pub ts_ms: i64,
    pub text: Option<String>,
    pub media: Option<ChatMediaMeta>,
    pub status: ChatStatus,
    /// 被引用消息的本端消息 id；None=无引用（加法字段，旧记录缺字段读回 None）。
    pub reply_to: Option<String>,
    /// 入群邀请卡片载荷（IMC1 加法字段，kind=groupinvite 时有值，其余恒 None）。
    pub card: Option<crate::ginvite::ChatInviteCard>,
}

pub use crate::friend::ChatFriend;
pub use crate::kind::{validate_media, ChatKind};

/// 发送附件入参（GUI 侧解码 base64 后传入）。
#[derive(Clone, Debug)]
pub struct ChatMediaInput {
    pub name: String,
    pub mime: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSendReport {
    pub message: ChatEnvelope,
    pub delivered: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    #[error("IO 错误：{0}")]
    Io(#[from] std::io::Error),
    #[error("JSON 错误：{0}")]
    Json(#[from] serde_json::Error),
    #[error("PeerId 非法：{0}")]
    InvalidPeer(String),
    #[error("不能与自己通信：{0}")]
    SelfPeer(String),
    #[error("文本非法：{0}")]
    InvalidText(String),
    #[error("昵称非法：{0}")]
    InvalidNickname(String),
    #[error("附件过大：{0} 字节（上限 64MiB）")]
    MediaTooLarge(u64),
    #[error("附件非法：{0}")]
    InvalidMedia(String),
    #[error("地址非法：{0}")]
    InvalidAddr(String),
    #[error("协议违规：{0}")]
    Protocol(String),
    #[error("连接失败：{0}")]
    ConnectFailed(String),
    #[error("发送失败：{0}")]
    SendFailed(String),
    #[error("流失败：{0}")]
    StreamFailed(String),
    #[error("未找到：{0}")]
    NotFound(String),
    #[error("回复引用非法：{0}")]
    InvalidReply(String),
    #[error("身份被占用：{0}")]
    IdentityBusy(String),
    #[error("分组名非法：{0}")]
    InvalidGroup(String),
    #[error("更新参数非法：{0}")]
    InvalidUpdate(String),
    #[error("已是好友：{0}")]
    AlreadyFriends(String),
}

/// base58 → 32 字节 PeerId；编码或长度非法即 Err（可读中文）。
pub(crate) fn parse_peer_id(s: &str) -> Result<PeerId, ChatError> {
    let bytes = bs58::decode(s)
        .into_vec()
        .map_err(|e| ChatError::InvalidPeer(format!("不是合法 base58：{s}（{e}）")))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| ChatError::InvalidPeer(format!("长度非法（应为 32 字节）：{s}")))?;
    Ok(PeerId::from_bytes(arr))
}

/// 文本校验：trim 后 1..=2000 字符，空串禁止发送。
pub fn validate_text(raw: &str) -> Result<String, ChatError> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(ChatError::InvalidText("文本为空，禁止发送".into()));
    }
    if t.chars().count() > MAX_TEXT_CHARS {
        return Err(ChatError::InvalidText(format!(
            "文本超过 {MAX_TEXT_CHARS} 字符上限"
        )));
    }
    Ok(t.to_string())
}

/// 昵称缺省回退：PeerId 前 8 字符 + "..."（F5：禁落 PeerId 原文；
/// accept 缺省、update --nickname 空串、自愈 display_name 同一口径）。
pub fn nickname_fallback(peer_id: &str) -> String {
    let head: String = peer_id.chars().take(8).collect();
    format!("{head}...")
}

/// 昵称校验：trim 后 ≤64 字符（空串允许，CLI/GUI 层回退 nickname_fallback）。
pub fn validate_nickname(raw: &str) -> Result<String, ChatError> {
    let t = raw.trim();
    if t.chars().count() > 64 {
        return Err(ChatError::InvalidNickname("昵称超过 64 字符上限".into()));
    }
    Ok(t.to_string())
}

/// 附件文件名 sanitize：仅保留 [A-Za-z0-9._-]，空/纯点/超长回退或截断。
pub fn sanitize_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .take(128)
        .collect();
    if cleaned.is_empty() || cleaned.chars().all(|c| c == '.') {
        "attachment".to_string()
    } else {
        cleaned
    }
}

/// 当前毫秒时间戳（发端本地时间）。
pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_validation_matrix() {
        assert_eq!(validate_text("  hello  ").unwrap(), "hello");
        assert!(validate_text("").is_err());
        assert!(validate_text("   ").is_err());
        assert!(validate_text(&"a".repeat(2001)).is_err());
        assert!(validate_text(&"a".repeat(2000)).is_ok());
        assert!(validate_text(&"汉".repeat(2000)).is_ok());
    }

    #[test]
    fn nickname_fallback_abbreviates_peer_id() {
        assert_eq!(nickname_fallback("abcdEFGH1234"), "abcdEFGH...");
        assert_eq!(nickname_fallback("短"), "短...");
    }

    #[test]
    fn nickname_validation() {
        assert_eq!(validate_nickname(" 小 b ").unwrap(), "小 b");
        assert!(validate_nickname("").is_ok());
        assert!(validate_nickname(&"a".repeat(65)).is_err());
    }

    #[test]
    fn sanitize_name_matrix() {
        assert_eq!(sanitize_name("photo.png"), "photo.png");
        // 仅去路径分隔符/控制字符，点号保留；纯点串回退 attachment
        assert_eq!(sanitize_name("a\\bc"), "abc");
        assert_eq!(sanitize_name("../../etc/passwd"), "....etcpasswd");
        assert_eq!(sanitize_name(""), "attachment");
        assert_eq!(sanitize_name("..."), "attachment");
        assert_eq!(sanitize_name("中文名.txt"), ".txt");
        assert_eq!(sanitize_name(&"x".repeat(500)).len(), 128);
    }

    #[test]
    fn peer_id_validation() {
        let kp = p2p_identity::Keypair::generate();
        let pid = kp.peer_id().to_string();
        assert!(parse_peer_id(&pid).is_ok());
        assert!(parse_peer_id("!!!not-base58!!!").is_err());
        assert!(parse_peer_id("hello").is_err());
    }
}
