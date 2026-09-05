//! 好友簿条目模型、分组校验与更新补丁（契约 gui-contract.md §12.1，IM-T43）。
//!
//! 从 model.rs 拆出（该文件行数红线）；序列化形状逐字对齐契约：字段 camelCase，
//! Option 序列化 null。组名校验与 GUI/mock 三方同口径：trim 后 1..=32 字符。

use serde::{Deserialize, Serialize};

use crate::model::{self, ChatError};
use crate::Chat;

/// 组名上限（trim 后字符数）。
pub const MAX_GROUP_CHARS: usize = 32;

/// 好友簿条目（friends.json 数组元素）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatFriend {
    #[serde(rename = "peerId")]
    pub peer_id: String,
    pub nickname: String,
    pub addrs: Vec<String>,
    pub note: Option<String>,
    /// 分组名；None = 未分组。加法字段（IM-T43）：旧记录缺字段经 serde(default)
    /// 读回 None；空串在写入前归一化为 None，落盘永不出现空串组名。
    #[serde(default)]
    pub group: Option<String>,
}

/// friend_update 补丁（IM-T43 + PR1 --addr）：group/nickname/note/addrs 至少一项，
/// 皆 None 拒绝；peer 不在簿 Err；group 空串 = 移出分组；addrs 整组替换（校验同 add）。
#[derive(Clone, Debug, Default)]
pub struct FriendPatch {
    /// Some(v) = 修改分组；空串归一化为 None（移出分组）。
    pub group: Option<String>,
    /// Some(v) = 修改昵称（validate_nickname 校验）。
    pub nickname: Option<String>,
    /// Some(v) = 修改备注；trim 后空串归一化为 None。
    pub note: Option<String>,
    /// Some(v) = 整组替换可拨地址（格式同 friend_add --addr，非法地址拒绝）。
    pub addrs: Option<Vec<String>>,
}

impl FriendPatch {
    /// 全部未提供 → false（friend_update 拒绝空补丁）。
    pub fn is_empty(&self) -> bool {
        self.group.is_none()
            && self.nickname.is_none()
            && self.note.is_none()
            && self.addrs.is_none()
    }
}

/// 组名校验：trim 后 1..=32 字符；空串/None 归一化为 Ok(None)（未分组）。
pub fn validate_group(raw: Option<&str>) -> Result<Option<String>, ChatError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let t = raw.trim();
    if t.is_empty() {
        return Ok(None);
    }
    if t.chars().count() > MAX_GROUP_CHARS {
        return Err(ChatError::InvalidGroup(format!(
            "分组名超过 {MAX_GROUP_CHARS} 字符上限"
        )));
    }
    Ok(Some(t.to_string()))
}

impl Chat {
    pub fn friend_update(
        &self,
        peer_id: &str,
        patch: &FriendPatch,
    ) -> Result<ChatFriend, ChatError> {
        let peer = model::parse_peer_id(peer_id)?;
        if patch.is_empty() {
            return Err(ChatError::InvalidUpdate(
                "更新内容为空：group/nickname/note/addrs 至少提供一项".into(),
            ));
        }
        let group = validate_group(patch.group.as_deref())?;
        if let Some(addrs) = &patch.addrs {
            for addr in addrs {
                self.core
                    .node
                    .add_peer_address(peer, addr)
                    .map_err(|e| ChatError::InvalidAddr(format!("{addr}: {e}")))?;
            }
        }
        let nickname = patch
            .nickname
            .as_deref()
            .map(model::validate_nickname)
            .transpose()?;
        let note = match patch.note.as_deref() {
            Some(n) if n.trim().is_empty() => Some(None),
            Some(n) => Some(Some(n.to_string())),
            None => None,
        };
        let mut friends = self.core.store.friends_list()?;
        let slot = friends
            .iter_mut()
            .find(|f| f.peer_id == peer_id)
            .ok_or_else(|| ChatError::NotFound(format!("好友不在簿：{peer_id}")))?;
        if patch.group.is_some() {
            slot.group = group;
        }
        if let Some(name) = nickname {
            slot.nickname = name;
        }
        if let Some(value) = note {
            slot.note = value;
        }
        if let Some(addrs) = &patch.addrs {
            slot.addrs = addrs.clone();
        }
        let updated = slot.clone();
        self.core.store.upsert_friend(updated.clone())?;
        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_validation_matrix() {
        assert_eq!(
            validate_group(Some("  同事 ")).unwrap().as_deref(),
            Some("同事")
        );
        assert_eq!(
            validate_group(Some("")).unwrap(),
            None,
            "空串归一化为未分组"
        );
        assert_eq!(
            validate_group(Some("   ")).unwrap(),
            None,
            "纯空白归一化为未分组"
        );
        assert_eq!(validate_group(None).unwrap(), None);
        assert!(validate_group(Some(&"a".repeat(33))).is_err());
        assert!(validate_group(Some(&"汉".repeat(32))).is_ok());
        assert!(
            validate_group(Some(&"汉".repeat(33))).is_err(),
            "按字符数计"
        );
    }

    #[test]
    fn patch_empty_detection() {
        assert!(FriendPatch::default().is_empty());
        assert!(!FriendPatch {
            group: Some(String::new()),
            ..Default::default()
        }
        .is_empty());
        assert!(!FriendPatch {
            note: Some("x".into()),
            ..Default::default()
        }
        .is_empty());
    }

    #[test]
    fn friend_group_camel_case_and_missing_field_tolerance() {
        let friend = ChatFriend {
            peer_id: "p".into(),
            nickname: "n".into(),
            addrs: vec![],
            note: None,
            group: Some("同事".into()),
        };
        let value = serde_json::to_value(&friend).expect("serialize");
        assert_eq!(value["group"], "同事", "字段名逐字为 group");
        let back: ChatFriend = serde_json::from_value(value).expect("deserialize");
        assert_eq!(back, friend, "roundtrip 不保真");

        // 旧格式（无 group 字段，T43 之前的历史记录）缺字段读回 = 未分组
        let legacy = serde_json::json!({
            "peerId": "p", "nickname": "n", "addrs": [], "note": null
        });
        let parsed: ChatFriend = serde_json::from_value(legacy).expect("旧记录必须可读");
        assert_eq!(parsed.group, None, "缺字段读回 None");
    }
}
