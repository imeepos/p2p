//! 聊天事件（契约 gui-contract.md §12.2 判别联合形状）。
//! 从 model.rs 拆出（该文件行数红线）；邀请事件随邀请制加好友新增。

use serde::{Deserialize, Serialize};

use crate::ginvite::GroupInvite;
use crate::invite::InviteState;
use crate::model::{ChatEnvelope, ChatStatus};

/// chat_message / chat_status / chat_invite 事件。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ChatEvent {
    #[serde(rename = "chat_message")]
    ChatMessage { peer: String, message: ChatEnvelope },
    #[serde(rename = "chat_status")]
    ChatStatus {
        peer: String,
        #[serde(rename = "messageId")]
        message_id: String,
        status: ChatStatus,
    },
    /// 邀请制加好友事件：incoming = 收到邀请；accepted = 对方同意（好友建立）；
    /// rejected = 对方拒绝。state 序列化为小写（契约 §12.2）。
    #[serde(rename = "chat_invite")]
    ChatInvite { peer: String, state: InviteState },
    /// 入群邀请（同意制，IMC1）事件：载荷 = 邀请条目（含 direction/state）。
    /// pending = 收到/刷新邀请或发起/重发；accepted = 收敛完成；rejected = 拒绝。
    #[serde(rename = "chat_group_invite")]
    GroupInvite { invite: GroupInvite },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_invite_event_tag_and_state_shape() {
        let value = serde_json::to_value(ChatEvent::ChatInvite {
            peer: "p".into(),
            state: InviteState::Incoming,
        })
        .expect("serialize");
        assert_eq!(value["type"], "chat_invite");
        assert_eq!(value["peer"], "p");
        assert_eq!(value["state"], "incoming");
    }

    #[test]
    fn chat_group_invite_event_wraps_full_entry() {
        let ev = ChatEvent::GroupInvite {
            invite: crate::ginvite::GroupInvite {
                id: "i1".into(),
                group_id: "g1".into(),
                group_name: "群".into(),
                owner: "o".into(),
                inviter: "o".into(),
                invitee: "e".into(),
                note: None,
                direction: crate::ginvite::GroupInviteDirection::In,
                state: crate::ginvite::GroupInviteState::Pending,
                ts_ms: 5,
                delivered: false,
            },
        };
        let value = serde_json::to_value(ev).expect("serialize");
        assert_eq!(value["type"], "chat_group_invite", "事件 tag 逐字一致");
        assert_eq!(value["invite"]["groupId"], "g1", "载荷 = 邀请条目");
        assert_eq!(value["invite"]["direction"], "in");
        assert_eq!(value["invite"]["state"], "pending");
    }
}
