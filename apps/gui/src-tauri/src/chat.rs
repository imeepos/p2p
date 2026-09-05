//! gui-contract.md §12 chat 命令面（T32）：p2p-chat 门面的 Tauri 薄封装。
//!
//! 返回类型直接复用 p2p-chat 的 serde 形状（字段 camelCase、Option→null，与 §12.3 逐字一致）；
//! 入参校验（peerId/昵称/addr/文本/媒体）全部在 crate 内完成，命令层只做 base64 解码与
//! 尺寸预检；Err 一律可读中文（ChatError Display 即中文，节点未启动由 state.chat() 兜底）。

use base64::Engine;
use p2p_chat::{ChatFriend, ChatKind, ChatSendReport};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

/// 契约 §12.3 ChatMediaInput：前端入参（dataBase64 = 原始字节 base64）。
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMediaInputJson {
    pub name: String,
    pub mime: String,
    pub data_base64: String,
}

/// chat_media_file 返回（契约 §12.1：path/mime/name）。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMediaFileJson {
    pub path: String,
    pub mime: String,
    pub name: String,
}

/// chat_friends_list：读好友簿（无文件返回空数组）。
#[tauri::command]
pub async fn chat_friends_list(state: State<'_, AppState>) -> Result<Vec<ChatFriend>, String> {
    let chat = state.chat().await?;
    chat.friends_list().map_err(|e| e.to_string())
}

/// chat_friend_invite（邀请制加好友）：发邀请并登记挂起（out），同意前不建好友。
#[tauri::command]
pub async fn chat_friend_invite(
    state: State<'_, AppState>,
    peer_id: String,
    nickname: String,
    addrs: Vec<String>,
) -> Result<p2p_chat::InviteReport, String> {
    let chat = state.chat().await?;
    chat.friend_invite(&peer_id, &nickname, addrs, None)
        .await
        .map_err(|e| e.to_string())
}

/// chat_invites_list：邀请列表（out 待对方同意 / in 待本机处理）。
#[tauri::command]
pub async fn chat_invites_list(
    state: State<'_, AppState>,
) -> Result<Vec<p2p_chat::FriendInvite>, String> {
    let chat = state.chat().await?;
    chat.invites_list().map_err(|e| e.to_string())
}

/// chat_invite_accept：同意来邀——本机立即互为好友并回投 ACCEPT。
#[tauri::command]
pub async fn chat_invite_accept(
    state: State<'_, AppState>,
    peer_id: String,
    nickname: String,
) -> Result<ChatFriend, String> {
    let chat = state.chat().await?;
    chat.invite_accept(&peer_id, &nickname)
        .await
        .map_err(|e| e.to_string())
}

/// chat_invite_reject：拒绝来邀（通知对方尽力而为）。
#[tauri::command]
pub async fn chat_invite_reject(
    state: State<'_, AppState>,
    peer_id: String,
) -> Result<(), String> {
    let chat = state.chat().await?;
    chat.invite_reject(&peer_id).await.map_err(|e| e.to_string())
}

/// chat_invite_cancel：撤回本机待同意邀请。
#[tauri::command]
pub async fn chat_invite_cancel(
    state: State<'_, AppState>,
    peer_id: String,
) -> Result<bool, String> {
    let chat = state.chat().await?;
    chat.invite_cancel(&peer_id).await.map_err(|e| e.to_string())
}

/// chat_friend_update（IM-T43）：分组/昵称/备注补丁；空补丁与越界组名由 crate 校验拒绝；
/// GUI 契约 §12.1 不暴露 addrs 修改（PR1 起仅 CLI update --addr 提供），恒传 None；
/// peer 不在簿 Err。group 空串 = 移出分组（crate 内归一化 None）。
#[tauri::command]
pub async fn chat_friend_update(
    state: State<'_, AppState>,
    peer_id: String,
    group: Option<String>,
    nickname: Option<String>,
    note: Option<String>,
) -> Result<ChatFriend, String> {
    let chat = state.chat().await?;
    let patch = p2p_chat::FriendPatch {
        group,
        nickname,
        note,
        addrs: None,
    };
    chat.friend_update(&peer_id, &patch)
        .map_err(|e| e.to_string())
}

/// chat_friend_remove：幂等；never 在簿返回 false，不删消息历史。
#[tauri::command]
pub async fn chat_friend_remove(
    state: State<'_, AppState>,
    peer_id: String,
) -> Result<bool, String> {
    let chat = state.chat().await?;
    chat.friend_remove(&peer_id).map_err(|e| e.to_string())
}

/// chat_history：time desc 分页；limit 默认 50 上限 100（crate 内收敛）；beforeId 严格更早游标。
#[tauri::command]
pub async fn chat_history(
    state: State<'_, AppState>,
    peer: String,
    before_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<p2p_chat::ChatEnvelope>, String> {
    let chat = state.chat().await?;
    let msgs = chat
        .history(&peer, before_id.as_deref(), limit.unwrap_or(0))
        .map_err(|e| e.to_string())?;
    Ok(msgs.into_iter().map(crate::util::to_asset_media).collect())
}

/// chat_send：校验 → 信封 → 落 outbox → 尝试投递；delivered=true 表示实时送达。
/// replyTo（可选）：被引用消息的本端消息 id，原样透传；空白引用由 crate 校验拒绝。
#[tauri::command]
pub async fn chat_send(
    state: State<'_, AppState>,
    peer: String,
    kind: ChatKind,
    text: Option<String>,
    media: Option<ChatMediaInputJson>,
    reply_to: Option<String>,
) -> Result<ChatSendReport, String> {
    let chat = state.chat().await?;
    let media = media.map(decode_media_input).transpose()?;
    let mut report = chat
        .send(&peer, kind, text, media, reply_to)
        .await
        .map_err(|e| e.to_string())?;
    report.message = crate::util::to_asset_media(report.message);
    Ok(report)
}

/// chat_media_file：附件落盘绝对路径（仅本端展示用）；非媒体或不存在 → Err。
#[tauri::command]
pub async fn chat_media_file(
    state: State<'_, AppState>,
    peer: String,
    message_id: String,
) -> Result<ChatMediaFileJson, String> {
    let chat = state.chat().await?;
    let meta = chat
        .media_file(&peer, &message_id)
        .map_err(|e| e.to_string())?;
    let path = meta.path.ok_or_else(|| "附件路径缺失".to_string())?;
    Ok(ChatMediaFileJson {
        path: crate::util::to_asset_url(&path),
        mime: meta.mime,
        name: meta.name,
    })
}

/// base64 解码为 p2p-chat 媒体入参；解码前按长度粗估拒绝超限载荷（≤64MiB，契约 §12.1），
/// 解码失败返回可读中文 Err。mime/尺寸的精确校验仍由 crate validate_media 兜底。
pub(crate) fn decode_media_input(
    input: ChatMediaInputJson,
) -> Result<p2p_chat::ChatMediaInput, String> {
    if estimated_bytes(&input.data_base64) > p2p_chat::MAX_MESSAGE_SIZE {
        return Err("附件超过单条消息上限（64MiB）".to_string());
    }
    let data = base64::engine::general_purpose::STANDARD
        .decode(&input.data_base64)
        .map_err(|e| format!("附件 base64 解码失败: {e}"))?;
    Ok(p2p_chat::ChatMediaInput {
        name: input.name,
        mime: input.mime,
        data,
    })
}

/// base64 字符串的字节数安全上界：每组 4 字符至多 3 字节，余组至多 3 字节。
/// 仅用于超限粗估（防解码超大载荷浪费内存），精确上限由 crate validate_media 兜底。
fn estimated_bytes(b64: &str) -> u64 {
    (b64.len() as u64 / 4 + 1) * 3
}
