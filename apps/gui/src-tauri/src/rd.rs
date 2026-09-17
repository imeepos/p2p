//! rd 远程桌面命令面（M6B）：host 服务开关 + 审批队列 + viewer 会话 + 质量协商。
//!
//! 槽位语义与 tunnel_serve 同构：node_start 装配（host 默认关闭），
//! rd_host_start/stop 翻转运行期开关（rd-host HostState.enabled）。
//! host 画面源为合成源（SyntheticFactory，零系统权限；真实采集经后续 GUI
//! 采集并入），审批闸默认开（GUI 审批队列是本面的核心形态）。

use std::path::PathBuf;
use std::sync::Arc;

use p2p::Node;
use p2p_identity::PeerId;
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Mutex;

use crate::state::AppState;

mod defaults;
mod input;
mod relay;

/// host 状态快照（camelCase，GUI 三态渲染）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RdHostStatus {
    pub running: bool,
    pub require_approval: bool,
    pub session_count: u32,
    pub pending_approvals: Vec<String>,
    pub fps: u8,
}

/// viewer 会话状态快照。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RdViewerStatus {
    pub connected: bool,
    pub session_id: Option<String>,
}

/// rd 槽位：host（rd-host 装配，会话态默认关）+ viewer 会话 + 节点句柄。
#[derive(Default)]
pub struct RdSlot {
    host: Mutex<Option<Arc<rd_host::RdHost>>>,
    node: Mutex<Option<Arc<Node>>>,
    viewer: Mutex<Option<rd_viewer::ViewerSession>>,
}

impl RdSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// node_start 装配：注册 rd 处理器，服务默认关闭（需 rd_host_start 显式开启）；
    /// 初始质量档 fps 取配置（CC2 rdFps，越界回落 15）。
    pub async fn install(&self, node: &Arc<Node>, initial_fps: u8) {
        let config = rd_host::HostConfig {
            fs_root: default_fs_root(),
            require_approval: true,
            fps: defaults::sanitized_initial_fps(initial_fps),
            ..Default::default()
        };
        match rd_host::RdHost::with_config(
            node.clone(),
            Arc::new(rd_host::SyntheticFactory { w: 1280, h: 720 }),
            Arc::new(rd_input::recording::RecordingInjectorFactory::new()),
            Arc::new(rd_clipboard::memory::MemoryClipboardFactory::new()),
            config,
        ) {
            Ok(host) => {
                *self.host.lock().await = Some(Arc::new(host));
                *self.node.lock().await = Some(node.clone());
                tracing::info!("rd host 已装配（默认关闭，需 rd_host_start）");
            }
            Err(e) => tracing::error!(error = %e, "rd host 装配失败（不阻断节点启动）"),
        }
    }

    /// node_stop 卸载：关服务、停会话、清句柄。
    pub async fn clear(&self) {
        if let Some(host) = self.host.lock().await.take() {
            host.set_enabled(false);
            host.stop_all();
        }
        self.close_viewer().await;
        *self.node.lock().await = None;
    }

    /// 服务开启（审批闸按参数翻转）。
    pub fn start(&self, require_approval: bool) -> Result<(), String> {
        let host = self
            .host
            .blocking_lock()
            .clone()
            .ok_or_else(|| "节点未启动，rd host 不可用".to_string())?;
        host.set_require_approval(require_approval);
        host.set_enabled(true);
        Ok(())
    }

    /// 服务关闭：停全部活跃会话。
    pub fn stop(&self) -> Result<(), String> {
        let host = self
            .host
            .blocking_lock()
            .clone()
            .ok_or_else(|| "节点未启动，rd host 不可用".to_string())?;
        host.set_enabled(false);
        host.stop_all();
        Ok(())
    }

    /// 状态快照。
    pub fn status(&self) -> RdHostStatus {
        let host = self.host.blocking_lock().clone();
        match host {
            Some(h) => {
                let pending = h.pending_approvals();
                RdHostStatus {
                    running: h.state_enabled(),
                    require_approval: h.state_require_approval(),
                    session_count: h.session_count() as u32,
                    pending_approvals: pending.iter().map(|p| p.to_string()).collect(),
                    fps: h.active_fps(),
                }
            }
            None => RdHostStatus::default(),
        }
    }

    /// 审批通过（返回是否确有 pending 消费）。
    pub fn approve(&self, peer: &PeerId) -> bool {
        self.host
            .blocking_lock()
            .as_ref()
            .map(|h| h.approve(peer))
            .unwrap_or(false)
    }

    pub fn deny(&self, peer: &PeerId) -> bool {
        self.host
            .blocking_lock()
            .as_ref()
            .map(|h| h.deny(peer))
            .unwrap_or(false)
    }

    /// 质量协商（host 侧采纳，viewer 请求经控制通道同路径）。
    pub fn set_quality(&self, fps: u8, scale: u8, codec: u8) -> Result<(), String> {
        let host = self
            .host
            .blocking_lock()
            .clone()
            .ok_or_else(|| "节点未启动，rd host 不可用".to_string())?;
        host.set_quality(fps, scale, codec).map(|_| ())
    }

    /// viewer 会话：连接对端（握手 + 视频流）；帧经 sink 交付（None = 丢弃）。
    pub async fn connect_viewer_with_sink(
        &self,
        peer: PeerId,
        session_id: Option<String>,
        sink: Option<Arc<dyn rd_viewer::RenderSink>>,
    ) -> Result<RdViewerStatus, String> {
        let node = self
            .node
            .lock()
            .await
            .clone()
            .ok_or_else(|| "节点未启动".to_string())?;
        let sid = session_id.unwrap_or_else(random_session_id);
        let viewer = rd_viewer::RdViewer::new(node);
        let sink: Arc<dyn rd_viewer::RenderSink> = match sink {
            Some(s) => s,
            None => Arc::new(NoopSink),
        };
        let session = viewer
            .connect(peer, sid.clone(), sink)
            .await
            .map_err(|e| format!("rd 连接失败: {e}"))?;
        *self.viewer.lock().await = Some(session);
        Ok(RdViewerStatus {
            connected: true,
            session_id: Some(sid),
        })
    }

    /// viewer 会话关闭。
    pub async fn close_viewer(&self) {
        if let Some(s) = self.viewer.lock().await.take() {
            let _ = s.close().await;
        }
    }

    pub async fn viewer_status(&self) -> RdViewerStatus {
        let g = self.viewer.lock().await;
        match g.as_ref() {
            Some(_) => RdViewerStatus {
                connected: true,
                session_id: None,
            },
            None => RdViewerStatus::default(),
        }
    }
}

/// 无操作渲染 sink（无 Channel 的探测型连接兜底）。
struct NoopSink;
impl rd_viewer::RenderSink for NoopSink {
    fn on_frame(&self, _frame: rd_viewer::DecodedFrame) {}
}

fn random_session_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..16].to_string()
}

fn default_fs_root() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home {
        Some(h) => h.join("Downloads").join("RD"),
        None => PathBuf::from(".rd-files"),
    }
}

/// peer base58 解析（与 CLI/tunnel 同规则）。
fn parse_peer(raw: &str) -> Result<PeerId, String> {
    let bytes = bs58::decode(raw.trim())
        .into_vec()
        .map_err(|e| format!("peer 非 base58: {e}"))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("peer 长度非法（须 32 字节）: {raw}"))?;
    Ok(PeerId::from_bytes(arr))
}

// ---- tauri 命令面 ----

#[tauri::command]
pub async fn rd_host_start(
    state: State<'_, AppState>,
    require_approval: Option<bool>,
) -> Result<RdHostStatus, String> {
    // CC2：None 回落配置缺省 rdRequireApproval；Some(v) 为运行态覆盖。
    let approval = defaults::effective_require_approval(
        require_approval,
        state.config_get().rd_require_approval,
    );
    state.rd().start(approval)?;
    Ok(state.rd().status())
}

#[tauri::command]
pub async fn rd_host_stop(state: State<'_, AppState>) -> Result<RdHostStatus, String> {
    state.rd().stop()?;
    Ok(state.rd().status())
}

#[tauri::command]
pub async fn rd_host_status(state: State<'_, AppState>) -> Result<RdHostStatus, String> {
    Ok(state.rd().status())
}

#[tauri::command]
pub async fn rd_approve(state: State<'_, AppState>, peer: String) -> Result<bool, String> {
    let peer = parse_peer(&peer)?;
    Ok(state.rd().approve(&peer))
}

#[tauri::command]
pub async fn rd_deny(state: State<'_, AppState>, peer: String) -> Result<bool, String> {
    let peer = parse_peer(&peer)?;
    Ok(state.rd().deny(&peer))
}

#[tauri::command]
pub async fn rd_quality_set(
    state: State<'_, AppState>,
    fps: u8,
    scale: u8,
    codec: u8,
) -> Result<RdHostStatus, String> {
    state.rd().set_quality(fps, scale, codec)?;
    Ok(state.rd().status())
}

#[tauri::command]
pub async fn rd_viewer_connect(
    state: State<'_, AppState>,
    peer: String,
    session_id: Option<String>,
    on_frame: tauri::ipc::Channel,
) -> Result<RdViewerStatus, String> {
    let peer = parse_peer(&peer)?;
    let sink: Arc<dyn rd_viewer::RenderSink> = Arc::new(relay::WebviewSink::new(on_frame));
    state
        .rd()
        .connect_viewer_with_sink(peer, session_id, Some(sink))
        .await
}

#[tauri::command]
pub async fn rd_viewer_close(state: State<'_, AppState>) -> Result<RdViewerStatus, String> {
    state.rd().close_viewer().await;
    Ok(RdViewerStatus::default())
}

#[tauri::command]
pub async fn rd_viewer_status(state: State<'_, AppState>) -> Result<RdViewerStatus, String> {
    Ok(state.rd().viewer_status().await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_serde_camel_case() {
        let s = RdHostStatus {
            running: true,
            require_approval: true,
            session_count: 2,
            pending_approvals: vec!["peer-a".into()],
            fps: 15,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"running\":true"));
        assert!(json.contains("\"requireApproval\":true"));
        assert!(json.contains("\"sessionCount\":2"));
        assert!(json.contains("\"pendingApprovals\""));
    }

    #[test]
    fn peer_parse_valid_and_invalid() {
        // base58("rd-peer-test-bytes-0123456789abcdef") 手工构造 32 字节
        let bytes = [7u8; 32];
        let b58 = bs58::encode(bytes).into_string();
        assert!(parse_peer(&b58).is_ok());
        assert!(parse_peer("not-base58!!").is_err());
        assert!(parse_peer("abc").is_err(), "长度非法");
    }
}
