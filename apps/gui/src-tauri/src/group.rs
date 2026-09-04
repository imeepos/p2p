//! gui-contract.md §7 group 命令面（G4）：p2p-chat 群门面的 Tauri 薄封装。
//!
//! 返回类型复用 p2p-chat 的 serde 形状（GroupInfo/GroupMessage/GroupSendReport，
//! camelCase 与 §7 逐字一致）；校验全部在 crate 内完成，命令层只做 base64 解码
//! 与尺寸预检；Err 一律可读中文（节点未启动由 state.chat() 兜底）。

use base64::Engine;
use p2p_chat::{GroupInfo, GroupMessage, GroupSendReport};
use tauri::State;

pub use crate::chat::ChatMediaFileJson;
use crate::chat::{decode_media_input, ChatMediaInputJson};
use crate::state::AppState;

/// group_create：校验（成员 ⊆ 好友簿、≤32、不含本机、群名 trim 1..=64）后建群并推 roster。
#[tauri::command]
pub async fn group_create(
    state: State<'_, AppState>,
    name: String,
    member_ids: Vec<String>,
) -> Result<GroupInfo, String> {
    let chat = state.chat().await?;
    chat.group
        .group_create(&name, &member_ids)
        .await
        .map_err(|e| e.to_string())
}

/// group_list：全量群（含 left/kicked/disbanded，GUI 按 state 过滤/置底）。
#[tauri::command]
pub async fn group_list(state: State<'_, AppState>) -> Result<Vec<GroupInfo>, String> {
    let chat = state.chat().await?;
    Ok(chat.group.group_list())
}

/// group_invite（owner-only）：rev+1 推全体（含新成员）；离线成员 goutbox 补投。
#[tauri::command]
pub async fn group_invite(
    state: State<'_, AppState>,
    group_id: String,
    member_ids: Vec<String>,
) -> Result<GroupInfo, String> {
    let chat = state.chat().await?;
    chat.group
        .group_invite(&group_id, &member_ids)
        .await
        .map_err(|e| e.to_string())
}

/// group_kick（owner-only）：rev+1 推余员 + G_KICK(reason=kicked)。
#[tauri::command]
pub async fn group_kick(
    state: State<'_, AppState>,
    group_id: String,
    member_id: String,
) -> Result<GroupInfo, String> {
    let chat = state.chat().await?;
    chat.group
        .group_kick(&group_id, &member_id)
        .await
        .map_err(|e| e.to_string())
}

/// group_leave：本端 state=left（历史保留）；向 owner 发 G_LEAVE（离线补投）。
#[tauri::command]
pub async fn group_leave(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<GroupInfo, String> {
    let chat = state.chat().await?;
    chat.group
        .group_leave(&group_id)
        .await
        .map_err(|e| e.to_string())
}

/// group_rename（owner-only）：校验群名 → rev+1 推 roster。
#[tauri::command]
pub async fn group_rename(
    state: State<'_, AppState>,
    group_id: String,
    name: String,
) -> Result<GroupInfo, String> {
    let chat = state.chat().await?;
    chat.group
        .group_rename(&group_id, &name)
        .await
        .map_err(|e| e.to_string())
}

/// group_disband（owner-only）：rev+1，对全体成员发 G_KICK(reason=disbanded)。
#[tauri::command]
pub async fn group_disband(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<GroupInfo, String> {
    let chat = state.chat().await?;
    chat.group
        .group_disband(&group_id)
        .await
        .map_err(|e| e.to_string())
}

/// group_send：校验 → 历史落 pending → 每目标 goutbox → 串行 fan-out；
/// acked/recipients/delivered 供 GUI 展示送达明细。
#[tauri::command]
pub async fn group_send(
    state: State<'_, AppState>,
    group_id: String,
    kind: p2p_chat::ChatKind,
    text: Option<String>,
    media: Option<ChatMediaInputJson>,
    reply_to: Option<String>,
) -> Result<GroupSendReport, String> {
    let chat = state.chat().await?;
    let media = media.map(decode_media_input).transpose()?;
    let mut report = chat
        .group
        .group_send(&group_id, kind, text, media, reply_to)
        .await
        .map_err(|e| e.to_string())?;
    report.message = crate::util::to_asset_group_media(report.message);
    Ok(report)
}

/// group_history：time desc 分页；beforeId 游标；limit 默认 50 上限 100（同 1:1 语义）。
#[tauri::command]
pub async fn group_history(
    state: State<'_, AppState>,
    group_id: String,
    before_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<GroupMessage>, String> {
    let chat = state.chat().await?;
    let msgs = chat
        .group
        .group_history(&group_id, before_id.as_deref(), limit.unwrap_or(0))
        .map_err(|e| e.to_string())?;
    Ok(msgs
        .into_iter()
        .map(crate::util::to_asset_group_media)
        .collect())
}

/// group_media_file：附件 asset URL（MediaContent 只认 https:/blob:/data:/asset:）。
#[tauri::command]
pub async fn group_media_file(
    state: State<'_, AppState>,
    group_id: String,
    message_id: String,
) -> Result<ChatMediaFileJson, String> {
    let chat = state.chat().await?;
    let meta = chat
        .group
        .group_media_file(&group_id, &message_id)
        .map_err(|e| e.to_string())?;
    let path = meta.path.ok_or_else(|| "附件路径缺失".to_string())?;
    Ok(ChatMediaFileJson {
        path: crate::util::to_asset_url(&path),
        mime: meta.mime,
        name: meta.name,
    })
}
