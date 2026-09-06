//! gui-contract.md §11 入群邀请命令面（IMC2）：p2p-chat 同意制入群邀请门面的
//! Tauri 薄封装（自 group.rs 拆出，同 ginvite_api 拆分纪律）。
//!
//! 冻结契约：命令名/参数名/事件名逐字一致；返回类型直接复用 p2p-chat 的
//! GroupInvite serde 形状（camelCase、Option→null，与 §11 逐字相同）；
//! chat_group_invite 事件经 node_event.rs 既有映射透传（ChatEvent::GroupInvite
//! → NodeEventJson::ChatGroupInvite），本模块不另起通道。Err 一律可读中文
//! （ChatError Display 即中文，节点未启动由 state.chat() 兜底）。

use p2p_chat::GroupInvite;
use tauri::State;

use crate::state::AppState;

/// chat_group_invite_send（owner-only，同意制）：发起入群邀请。
/// 邀请人展示名取本机节点资料 name（空/非法由 crate 回退 PeerId 缩略，同
/// display_name 口径）；对端离线不失败：返回条目 delivered=false（挂起，
/// 重连重投），重复邀请幂等刷新（id 稳定）。
#[tauri::command]
pub async fn chat_group_invite_send(
    state: State<'_, AppState>,
    group_id: String,
    peer_id: String,
    note: Option<String>,
) -> Result<GroupInvite, String> {
    let chat = state.chat().await?;
    let inviter = state.profile_get().name;
    let report = chat
        .group
        .group_invite_member(&group_id, &peer_id, &inviter, note)
        .await
        .map_err(|e| e.to_string())?;
    // 门面把真实送达信号只放在 report.delivered（store 已 patch 而内存条目未刷新）；
    // 契约语义 delivered=邀请帧已送达（对端 ACK 为证），此处对齐后再出参。
    let mut invite = report.invite;
    invite.delivered = report.delivered;
    Ok(invite)
}

/// chat_group_invites_list：in + out 合一邀请列表（tsMs 倒序；无簿返回空数组）。
#[tauri::command]
pub async fn chat_group_invites_list(
    state: State<'_, AppState>,
) -> Result<Vec<GroupInvite>, String> {
    let chat = state.chat().await?;
    chat.group.group_invites_list().map_err(|e| e.to_string())
}

/// chat_group_invite_accept：受邀者同意（仅 in+pending）；owner 离线时决策
/// 挂起（delivered=false），重连/重启重投收敛。契约返回 void。
#[tauri::command]
pub async fn chat_group_invite_accept(
    state: State<'_, AppState>,
    invite_id: String,
) -> Result<(), String> {
    let chat = state.chat().await?;
    chat.group
        .group_invite_accept(&invite_id)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// chat_group_invite_reject：受邀者拒绝（仅 in+pending；reason 可空随决策帧
/// 回送）。契约返回 void。
#[tauri::command]
pub async fn chat_group_invite_reject(
    state: State<'_, AppState>,
    invite_id: String,
    reason: Option<String>,
) -> Result<(), String> {
    let chat = state.chat().await?;
    chat.group
        .group_invite_reject(&invite_id, reason)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 命令层出参形状冻结断言：camelCase 字段逐字（防桥接层另造形状漂移）；
    /// 载荷本体序列化语义归 p2p-chat crate 测试，此处不重复。
    #[test]
    fn invite_output_shape_is_frozen_camel_case() {
        let invite = GroupInvite {
            id: "i1".into(),
            group_id: "g1".into(),
            group_name: "群".into(),
            owner: "o".into(),
            inviter: "o".into(),
            invitee: "e".into(),
            note: None,
            direction: p2p_chat::GroupInviteDirection::In,
            state: p2p_chat::GroupInviteState::Pending,
            ts_ms: 5,
            delivered: false,
        };
        let v = serde_json::to_value(&invite).expect("serialize");
        for key in [
            "id",
            "groupId",
            "groupName",
            "owner",
            "inviter",
            "invitee",
            "note",
            "direction",
            "state",
            "tsMs",
            "delivered",
        ] {
            assert!(v.get(key).is_some(), "缺冻结字段 {key}: {v}");
        }
        assert_eq!(v["note"], serde_json::Value::Null, "Option 序列化 null");
    }
}
