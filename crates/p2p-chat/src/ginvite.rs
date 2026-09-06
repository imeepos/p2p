//! 入群邀请（同意制）模型：线协议 /im/ginvite/1（wire-protocol.md §8.4 登记）。
//! 序列化纪律同 invite.rs：camelCase、Option 序列化 null、加法字段 serde(default)。

use serde::{Deserialize, Serialize};

/// 邀请方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupInviteDirection {
    /// 本机（owner）发出，等待受邀者同意。
    Out,
    /// 本机（受邀者）收到，待本机处理。
    In,
}

/// 邀请生命周期（chat_group_invite 事件的 state）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupInviteState {
    /// 待处理：新邀请 / 刷新后待受邀者决策 / 决策已回送待收敛。
    Pending,
    /// 收敛完成：受邀者在列的 roster 已到（受邀者侧）/ 同意已处理（owner 侧）。
    Accepted,
    /// 已拒绝。
    Rejected,
}

/// 邀请簿上限：防 group_invites.json 无界膨胀；重复邀请 upsert 不新增条目。
pub const MAX_GROUP_INVITES: usize = 256;

/// 入群邀请条目（group_invites.json 数组元素，方向两侧同构）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInvite {
    pub id: String,
    pub group_id: String,
    pub group_name: String,
    /// 群主 PeerId（发起 owner-only，与 inviter 恒同，冗余承载防伪装）。
    pub owner: String,
    /// 邀请人 PeerId。
    pub inviter: String,
    /// 受邀人 PeerId。
    pub invitee: String,
    pub note: Option<String>,
    pub direction: GroupInviteDirection,
    pub state: GroupInviteState,
    #[serde(rename = "tsMs")]
    pub ts_ms: i64,
    /// out = 邀请帧已送达（对端 ACK 为证）；in = 本机决策帧已回送。
    /// 重复邀请/重新决策刷新为 false，重连重投由 outbox 联动。
    #[serde(default)]
    pub delivered: bool,
}

impl GroupInvite {
    /// upsert 归一键：同方向 + 同群 + 同对端组合视为同一条（刷新不新增）。
    pub(crate) fn same_slot(&self, other: &GroupInvite) -> bool {
        self.direction == other.direction
            && self.group_id == other.group_id
            && self.inviter == other.inviter
            && self.invitee == other.invitee
    }

    /// 帧通信对端：out 面向受邀者，in 面向邀请人。
    pub(crate) fn counterparty(&self) -> &str {
        match self.direction {
            GroupInviteDirection::Out => &self.invitee,
            GroupInviteDirection::In => &self.inviter,
        }
    }
}

/// 1:1 卡片消息载荷（ChatEnvelope.card，kind=groupinvite）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatInviteCard {
    pub group_id: String,
    pub group_name: String,
    pub inviter_nickname: String,
    pub note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(direction: GroupInviteDirection) -> GroupInvite {
        GroupInvite {
            id: "i1".into(),
            group_id: "g1".into(),
            group_name: "群".into(),
            owner: "o".into(),
            inviter: "o".into(),
            invitee: "e".into(),
            note: None,
            direction,
            state: GroupInviteState::Pending,
            ts_ms: 1,
            delivered: false,
        }
    }

    #[test]
    fn invite_camel_case_roundtrip() {
        let invite = fixture(GroupInviteDirection::In);
        let value = serde_json::to_value(&invite).expect("serialize");
        assert_eq!(value["groupId"], "g1", "字段名逐字为 groupId");
        assert_eq!(value["groupName"], "群", "字段名逐字为 groupName");
        assert_eq!(value["tsMs"], 1, "字段名逐字为 tsMs");
        assert_eq!(value["direction"], "in");
        assert_eq!(value["state"], "pending");
        let back: GroupInvite = serde_json::from_value(value).expect("deserialize");
        assert_eq!(back, invite, "roundtrip 不保真");
    }

    #[test]
    fn additive_fields_default_and_note_null() {
        let value = serde_json::to_value(fixture(GroupInviteDirection::Out)).expect("serialize");
        assert_eq!(value["note"], serde_json::Value::Null, "Option 序列化 null");
        let legacy = serde_json::json!({
            "id": "i1", "groupId": "g1", "groupName": "群", "owner": "o",
            "inviter": "o", "invitee": "e", "note": null,
            "direction": "out", "state": "pending", "tsMs": 1
        });
        let parsed: GroupInvite = serde_json::from_value(legacy).expect("旧记录必须可读");
        assert!(!parsed.delivered, "旧记录缺 delivered 读回 false");
    }

    #[test]
    fn same_slot_is_direction_and_party_scoped() {
        let out = fixture(GroupInviteDirection::Out);
        let mut in_rev = fixture(GroupInviteDirection::In);
        in_rev.invitee = "o".into();
        in_rev.inviter = "e".into();
        assert!(out.same_slot(&out), "同条目命中");
        assert!(!out.same_slot(&in_rev), "方向/对端不同不命中");
    }

    #[test]
    fn counterparty_follows_direction() {
        assert_eq!(fixture(GroupInviteDirection::Out).counterparty(), "e");
        assert_eq!(fixture(GroupInviteDirection::In).counterparty(), "o");
    }
}
