//! 好友邀请模型（邀请制加好友）：out = 本机已发出待对方同意；in = 对方发来待本机处理。
//! 序列化纪律同 friend.rs：camelCase、Option 序列化 null、加法字段 serde(default)。

use serde::{Deserialize, Serialize};

/// 邀请方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InviteDirection {
    /// 本机发出，等待对方同意。
    Out,
    /// 对方发来，等待本机同意或拒绝。
    In,
}

/// 邀请生命周期（chat_invite 事件的 state 字段）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InviteState {
    /// 收到新邀请（或被对方刷新）。
    Incoming,
    /// 对方已同意，双向好友关系建立。
    Accepted,
    /// 对方已拒绝。
    Rejected,
}

/// 邀请簿上限：防 invites.json 无界膨胀；重复邀请走 upsert 不新增条目。
pub const MAX_INVITES: usize = 256;

/// 好友邀请条目（invites.json 数组元素）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendInvite {
    #[serde(rename = "peerId")]
    pub peer_id: String,
    /// INVITE 帧里对端自称昵称（in）或本机为对方预备的显示名（out）。
    pub nickname: String,
    /// in = 对端自报可回拨地址；out = 本机已知的对端地址（可拨性登记）。
    pub addrs: Vec<String>,
    pub note: Option<String>,
    pub direction: InviteDirection,
    #[serde(rename = "tsMs")]
    pub ts_ms: i64,
    /// 已成功送达对端（ACK 为证）：防 PeerConnected 重投竞态；重复邀请刷新为 false。
    #[serde(default)]
    pub delivered: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(direction: InviteDirection) -> FriendInvite {
        FriendInvite {
            peer_id: "p".into(),
            nickname: "n".into(),
            addrs: vec!["127.0.0.1/u1".into()],
            note: None,
            direction,
            ts_ms: 1,
            delivered: false,
        }
    }

    #[test]
    fn direction_lower_case_shape() {
        assert_eq!(
            serde_json::to_value(InviteDirection::Out).unwrap(),
            serde_json::json!("out")
        );
        assert_eq!(
            serde_json::to_value(InviteState::Incoming).unwrap(),
            serde_json::json!("incoming")
        );
    }

    #[test]
    fn invite_camel_case_roundtrip() {
        let invite = fixture(InviteDirection::In);
        let value = serde_json::to_value(&invite).expect("serialize");
        assert_eq!(value["peerId"], "p", "字段名逐字为 peerId");
        assert_eq!(value["tsMs"], 1, "字段名逐字为 tsMs");
        assert_eq!(value["direction"], "in");
        let back: FriendInvite = serde_json::from_value(value).expect("deserialize");
        assert_eq!(back, invite, "roundtrip 不保真");
    }

    #[test]
    fn note_missing_reads_back_none() {
        let legacy = serde_json::json!({
            "peerId": "p", "nickname": "n", "addrs": [],
            "direction": "out", "tsMs": 0
        });
        let parsed: FriendInvite = serde_json::from_value(legacy).expect("旧记录必须可读");
        assert_eq!(parsed.note, None);
        assert!(!parsed.delivered, "旧记录缺 delivered 读回 false");
    }
}
