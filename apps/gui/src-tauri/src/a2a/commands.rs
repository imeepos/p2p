//! a2a 命令壳：Tauri IPC 薄封装。

use tauri::command;

/// 列出全部 agent（本机发布的 + 远程发现的）。
#[command]
pub async fn a2a_list() -> Result<Vec<serde_json::Value>, String> {
    // TODO: 实现 a2a list
    Ok(vec![])
}

/// 发布 agent（创建或更新可见性）。
#[command]
pub async fn a2a_publish(
    agent_id: String,
    _name: String,
    _description: String,
    _visibility: String,
) -> Result<serde_json::Value, String> {
    // TODO: 实现 a2a publish
    Ok(serde_json::json!({ "agentId": agent_id }))
}

/// 下架 agent（删除）。
#[command]
pub async fn a2a_unpublish(agent_id: String) -> Result<serde_json::Value, String> {
    // TODO: 实现 a2a unpublish
    Ok(serde_json::json!({ "removed": agent_id }))
}

/// 授权 peer 访问 private agent。
#[command]
pub async fn a2a_allow(agent_id: String, peer_id: String) -> Result<serde_json::Value, String> {
    // TODO: 实现 a2a allow
    Ok(serde_json::json!({ "granted": { "agentId": agent_id, "peer": peer_id } }))
}

/// 撤销授权。
#[command]
pub async fn a2a_disallow(agent_id: String, peer_id: String) -> Result<serde_json::Value, String> {
    // TODO: 实现 a2a disallow
    Ok(serde_json::json!({ "revoked": { "agentId": agent_id, "peer": peer_id } }))
}
