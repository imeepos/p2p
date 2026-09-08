//! 对端节点资料模型（/im/profile/1 载荷，gui-contract §11 同构）。
//! 序列化纪律同 invite.rs：camelCase；长度上限对齐契约 §11（src-tauri 校验
//! MIME 白名单，本 crate 只做长度防线，越界即 Protocol 违规）。

use serde::{Deserialize, Serialize};

use crate::model::ChatError;

/// name 上限（字符数，契约 §11）。
pub const NAME_MAX_CHARS: usize = 64;
/// description 上限（字符数，契约 §11）。
pub const DESCRIPTION_MAX_CHARS: usize = 280;
/// avatar data URL 总长上限（ASCII 字符数，契约 §11）。
pub const AVATAR_MAX_LEN: usize = 200_000;

/// 对端节点资料（响应载荷）。全空 = 对端未设置资料。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PeerProfile {
    pub name: String,
    pub description: String,
    /// data URL；None = 未设置。
    pub avatar: Option<String>,
}

impl PeerProfile {
    /// 长度防线（收发两向共用）：任一字段越界即协议违规。
    pub fn validate(&self) -> Result<(), ChatError> {
        if self.name.chars().count() > NAME_MAX_CHARS {
            return Err(ChatError::Protocol(format!(
                "对端资料 name 超过 {NAME_MAX_CHARS} 字符上限"
            )));
        }
        if self.description.chars().count() > DESCRIPTION_MAX_CHARS {
            return Err(ChatError::Protocol(format!(
                "对端资料 description 超过 {DESCRIPTION_MAX_CHARS} 字符上限"
            )));
        }
        if let Some(avatar) = &self.avatar {
            if avatar.len() > AVATAR_MAX_LEN {
                return Err(ChatError::Protocol(format!(
                    "对端资料 avatar 超过 {AVATAR_MAX_LEN} 字符上限"
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_accepts_bounds_and_rejects_overrun() {
        let ok = PeerProfile {
            name: "n".repeat(NAME_MAX_CHARS),
            description: "d".repeat(DESCRIPTION_MAX_CHARS),
            avatar: Some("a".repeat(AVATAR_MAX_LEN)),
        };
        assert!(ok.validate().is_ok());

        let long_name = PeerProfile {
            name: "名".repeat(NAME_MAX_CHARS + 1),
            ..PeerProfile::default()
        };
        assert!(long_name.validate().is_err());

        let long_desc = PeerProfile {
            description: "述".repeat(DESCRIPTION_MAX_CHARS + 1),
            ..PeerProfile::default()
        };
        assert!(long_desc.validate().is_err());

        let long_avatar = PeerProfile {
            avatar: Some("x".repeat(AVATAR_MAX_LEN + 1)),
            ..PeerProfile::default()
        };
        assert!(long_avatar.validate().is_err());
    }

    #[test]
    fn serde_camel_case_roundtrip() {
        let profile = PeerProfile {
            name: "甲".into(),
            description: "自我介绍".into(),
            avatar: None,
        };
        let text = serde_json::to_string(&profile).unwrap();
        assert!(text.contains("\"description\":\"自我介绍\""));
        // 序列化纪律同 friend.rs：Option 序列化 null
        assert!(text.contains("\"avatar\":null"));
        let back: PeerProfile = serde_json::from_str(&text).unwrap();
        assert_eq!(back, profile);
    }
}
