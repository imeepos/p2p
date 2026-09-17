//! 静态对端簿命令面（W2b/CC4）：gui-contract §3.1 本机服务配置面。
//!
//! 存储复用 p2p::static_peers::StaticPeersFile（0600、tmp+rename、按 peerId
//! 去重；本波提为 pub）；数据根 = app 数据目录（§18 口径）。效果语义：命令面
//! 只触文件不触运行中节点，消费方 = p2p 装配层（NodeBuilder.static_peers_file
//! 装配载入为 Manual 来源地址簿），修改后节点重启拨号生效（补料已接线，接线点
//! = state/node_build.rs 消费 wirable_book，CLI daemon.rs 同款语义）。

use std::path::{Path, PathBuf};

use p2p::static_peers::StaticPeersFile;
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

/// 文件名复用 p2p 单一约定（命令面与装配接线同根同名，防漂移）。
pub(crate) const FILE_NAME: &str = p2p::static_peers::FILE_NAME;

/// 单条静态对端（camelCase 逐字：peerId/addrs/note）。
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticPeerJson {
    pub peer_id: String,
    pub addrs: Vec<String>,
    pub note: String,
}

/// static_peers_list 回包。
#[derive(Debug, PartialEq, Serialize)]
pub struct StaticPeersListReport {
    pub peers: Vec<StaticPeerJson>,
}

/// static_peers_list：缺失 = 空册；损坏 = 显式 Err 不静默。
#[tauri::command]
pub async fn static_peers_list(
    state: State<'_, AppState>,
) -> Result<StaticPeersListReport, String> {
    list_peers(&book_path(state.data_dir())).map(|peers| StaticPeersListReport { peers })
}

/// static_peers_upsert：按 peerId 去重覆盖（整册重写 0600）。
#[tauri::command]
pub async fn static_peers_upsert(
    state: State<'_, AppState>,
    peer_id: String,
    addrs: Vec<String>,
    note: String,
) -> Result<bool, String> {
    upsert_peer(&book_path(state.data_dir()), peer_id, addrs, note)?;
    Ok(true)
}

/// static_peers_remove：幂等，不存在亦成功（契约逐字）。
#[tauri::command]
pub async fn static_peers_remove(
    state: State<'_, AppState>,
    peer_id: String,
) -> Result<bool, String> {
    remove_peer(&book_path(state.data_dir()), &peer_id)?;
    Ok(true)
}

fn book_path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE_NAME)
}

/// 装配接线缝（W2b 补料）：委托 p2p 单一 fail-safe 实现（缺失/损坏 = None，
/// 损坏留 warn），供 state/node_build.rs 消费。
pub(crate) fn wirable_book(data_dir: &Path) -> Option<PathBuf> {
    p2p::static_peers::wirable_path(data_dir)
}

fn open_book(path: &Path) -> Result<StaticPeersFile, String> {
    StaticPeersFile::load(path.to_path_buf())
        .map_err(|e| format!("读取 static-peers.json 失败: {e}"))
}

fn list_peers(path: &Path) -> Result<Vec<StaticPeerJson>, String> {
    Ok(open_book(path)?
        .entries()
        .into_iter()
        .map(|e| StaticPeerJson {
            peer_id: e.peer_id,
            addrs: e.addrs,
            note: e.note,
        })
        .collect())
}

fn upsert_peer(
    path: &Path,
    peer_id: String,
    addrs: Vec<String>,
    note: String,
) -> Result<(), String> {
    if peer_id.trim().is_empty() {
        return Err("peerId 不能为空".into());
    }
    open_book(path)?
        .upsert(peer_id, addrs, note)
        .map_err(|e| format!("写入 static-peers.json 失败: {e}"))
}

fn remove_peer(path: &Path, peer_id: &str) -> Result<(), String> {
    open_book(path)?
        .remove(peer_id)
        .map_err(|e| format!("写入 static-peers.json 失败: {e}"))
}

#[cfg(test)]
mod tests;
