//! 消息 kind 枚举与媒体校验（自 model.rs 拆出：行数红线再平衡）。
//! serde 小写、Display 同名；校验失败一律可读中文 Err，禁止静默降级。

use serde::{Deserialize, Serialize};

use crate::model::{ChatError, MAX_MESSAGE_SIZE};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatKind {
    Text,
    Image,
    Audio,
    Video,
    File,
    /// 入群邀请卡片（1:1 卡片消息，IMC1）：载荷在 ChatEnvelope.card，不接受附件；
    /// 仅随群门面邀请流程发出，公开 send() 显式拒绝该 kind。
    GroupInvite,
}

impl std::fmt::Display for ChatKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ChatKind::Text => "text",
            ChatKind::Image => "image",
            ChatKind::Audio => "audio",
            ChatKind::Video => "video",
            ChatKind::File => "file",
            ChatKind::GroupInvite => "groupinvite",
        })
    }
}

/// MIME 白名单按 kind 校验：mime 小写后精确匹配，不匹配 Err 不降级。
/// groupinvite 卡片白名单为空集：任何附件一律拒绝（卡片只承载载荷字段）。
pub fn validate_media(kind: &ChatKind, mime: &str, size: u64) -> Result<(), ChatError> {
    if size > MAX_MESSAGE_SIZE {
        return Err(ChatError::MediaTooLarge(size));
    }
    if size == 0 {
        return Err(ChatError::InvalidMedia("附件字节为空".into()));
    }
    let mime = mime.trim().to_ascii_lowercase();
    let allowed: &[&str] = match kind {
        ChatKind::Image => &["image/png", "image/jpeg", "image/gif", "image/webp"],
        ChatKind::Audio => &[
            "audio/mpeg",
            "audio/wav",
            "audio/ogg",
            "audio/m4a",
            "audio/mp4",
        ],
        ChatKind::Video => &["video/mp4", "video/webm", "video/mov", "video/quicktime"],
        ChatKind::File | ChatKind::Text => return Ok(()),
        ChatKind::GroupInvite => &[],
    };
    if !allowed.contains(&mime.as_str()) {
        return Err(ChatError::InvalidMedia(format!(
            "MIME 与 kind 不匹配：{kind} 不接受 {mime}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_validation_matrix() {
        assert!(validate_media(&ChatKind::Image, "image/png", 1).is_ok());
        assert!(validate_media(&ChatKind::Image, "image/webp", 1).is_ok());
        assert!(validate_media(&ChatKind::Image, "IMAGE/JPEG", 1).is_ok());
        assert!(validate_media(&ChatKind::Image, "image/svg+xml", 1).is_err());
        assert!(validate_media(&ChatKind::Audio, "audio/mpeg", 1).is_ok());
        assert!(validate_media(&ChatKind::Audio, "audio/mp4", 1).is_ok());
        assert!(validate_media(&ChatKind::Video, "video/quicktime", 1).is_ok());
        assert!(validate_media(&ChatKind::Video, "video/mp4", 1).is_ok());
        assert!(validate_media(&ChatKind::File, "application/octet-stream", 1).is_ok());
        assert!(validate_media(&ChatKind::File, "text/plain", 1).is_ok());
        assert!(validate_media(&ChatKind::Image, "image/png", MAX_MESSAGE_SIZE + 1).is_err());
        assert!(validate_media(&ChatKind::Image, "image/png", 0).is_err());
        assert!(validate_media(&ChatKind::Image, "image/png", MAX_MESSAGE_SIZE).is_ok());
    }

    #[test]
    fn group_invite_kind_carries_no_media_and_matches_display() {
        assert!(validate_media(&ChatKind::GroupInvite, "image/png", 1).is_err());
        assert!(validate_media(&ChatKind::GroupInvite, "application/octet-stream", 1).is_err());
        assert_eq!(ChatKind::GroupInvite.to_string(), "groupinvite");
        assert_eq!(
            serde_json::to_value(ChatKind::GroupInvite).expect("serialize"),
            serde_json::json!("groupinvite"),
            "kind serde 形状与 Display 逐字一致"
        );
    }
}
