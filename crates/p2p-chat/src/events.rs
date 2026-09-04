//! 聊天事件（契约 gui-contract.md §12.2 判别联合形状）。
//! 从 model.rs 拆出（该文件行数红线）；邀请事件随邀请制加好友新增。

use serde::{Deserialize, Serialize};

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
    ChatInvite {
        peer: String,
        state: InviteState,
    },
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
}
