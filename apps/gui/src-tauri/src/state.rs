//! 节点生命周期状态（gui-contract.md §1 语义）：唯一 running 槽位 + 持久化配置。
//!
//! 命令层薄封装，业务判断集中在此；AppHandle 不进本模块，保证可脱离 Tauri 单测。
//! peer 拨号/测距见子模块 peers。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use p2p::{Node, NodeEvent};
use tokio::sync::{broadcast, Mutex};
use tracing::warn;

use crate::config::ConfigStore;
use crate::proto;
use crate::types::{GuiConfig, MetricsJson, NodeStatus};

mod peers;

/// 运行中的节点及其生效配置。
pub(crate) struct RunningNode {
    node: Arc<Node>,
    config: GuiConfig,
    started_at: Instant,
    started_at_epoch_ms: u64,
}

/// node_start 产物：状态快照 + 事件接收端（转发任务由命令层接管）。
#[derive(Debug)]
pub struct StartedNode {
    pub status: NodeStatus,
    pub listen_addrs: Vec<String>,
    /// 启动即订阅，缩小启动窗口丢事件的可能。
    pub events: broadcast::Receiver<NodeEvent>,
}

/// 全局应用状态：Tauri managed。
pub struct AppState {
    running: Mutex<Option<RunningNode>>,
    config: ConfigStore,
}

impl AppState {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            running: Mutex::new(None),
            config: ConfigStore::new(app_data_dir),
        }
    }

    /// node_start：已运行 Err；成功后占槽并注册 echo handler、订阅事件。
    pub async fn start(&self, cfg: GuiConfig) -> Result<StartedNode, String> {
        let mut slot = self.running.lock().await;
        if slot.is_some() {
            return Err("节点已在运行，请勿重复启动".into());
        }
        let node = build_node(&cfg).await?;
        node.handle_protocol(Arc::new(proto::EchoHandler));
        let events = node.events();
        let listen_addrs = node.listen_addrs();
        let peer_id = node.local_peer_id().to_string();
        let started_at_epoch_ms = crate::util::now_ms();
        *slot = Some(RunningNode {
            node: Arc::new(node),
            config: cfg.clone(),
            started_at: Instant::now(),
            started_at_epoch_ms,
        });
        Ok(StartedNode {
            status: NodeStatus {
                running: true,
                peer_id: Some(peer_id),
                listen_addrs: listen_addrs.clone(),
                uptime_secs: 0,
                started_at_ms: Some(started_at_epoch_ms),
                config: cfg,
            },
            listen_addrs,
            events,
        })
    }

    /// node_stop：幂等，返回是否真的停掉了节点。
    pub async fn stop(&self) -> bool {
        let mut slot = self.running.lock().await;
        match slot.take() {
            Some(running) => {
                running.node.shutdown();
                true
            }
            None => false,
        }
    }

    /// 当前状态快照；未运行回持久化配置（契约 §3）。
    pub async fn status(&self) -> NodeStatus {
        let slot = self.running.lock().await;
        match slot.as_ref() {
            Some(r) => NodeStatus {
                running: true,
                peer_id: Some(r.node.local_peer_id().to_string()),
                listen_addrs: r.node.listen_addrs(),
                uptime_secs: r.started_at.elapsed().as_secs(),
                started_at_ms: Some(r.started_at_epoch_ms),
                config: r.config.clone(),
            },
            None => {
                let config = self.config.load();
                NodeStatus {
                    running: false,
                    peer_id: None,
                    listen_addrs: Vec::new(),
                    uptime_secs: 0,
                    started_at_ms: None,
                    config,
                }
            }
        }
    }

    /// metrics_get：未运行返回全零快照（前端按零值渲染）。
    pub async fn metrics(&self) -> MetricsJson {
        let slot = self.running.lock().await;
        match slot.as_ref() {
            Some(r) => MetricsJson::from(r.node.metrics()),
            None => MetricsJson::default(),
        }
    }

    pub fn config_get(&self) -> GuiConfig {
        self.config.load()
    }

    /// config_save：原子写盘，不触碰运行中节点（契约 §1）。
    pub fn config_save(&self, cfg: GuiConfig) -> Result<GuiConfig, String> {
        self.config.save(&cfg)?;
        Ok(cfg)
    }

    /// identity_reset：停节点（若在跑）+ 删身份数据目录内种子文件；confirm 校验在命令层。
    /// 返回（重置后状态, 是否停了运行中的节点）。
    pub async fn reset_identity(&self) -> Result<(NodeStatus, bool), String> {
        let (data_dir, was_running) = {
            let mut slot = self.running.lock().await;
            match slot.take() {
                Some(r) => (r.config.data_dir.clone(), true),
                None => (self.config.load().data_dir, false),
            }
        };
        remove_seed(Path::new(&data_dir))?;
        Ok((self.status().await, was_running))
    }

    /// 追加事件订阅（诊断/集成冒烟用；与命令层转发订阅互不影响）。
    pub async fn subscribe_events(&self) -> Result<broadcast::Receiver<NodeEvent>, String> {
        let slot = self.running.lock().await;
        slot.as_ref()
            .map(|r| r.node.events())
            .ok_or_else(|| "节点未运行，请先启动节点".into())
    }

    /// 取运行中节点的 Arc 克隆；未运行返回中文错误。
    pub(super) async fn running_node(&self) -> Result<Arc<Node>, String> {
        let slot = self.running.lock().await;
        slot.as_ref()
            .map(|r| r.node.clone())
            .ok_or_else(|| "节点未运行，请先启动节点".into())
    }
}

/// GuiConfig → NodeBuilder 装配（契约 §1 node_start）。
async fn build_node(cfg: &GuiConfig) -> Result<Node, String> {
    let mut builder = Node::builder()
        .quic_port(cfg.quic_port)
        .tcp_port(cfg.tcp_port)
        .bootstrap(cfg.bootstrap.clone())
        .mdns(cfg.enable_mdns)
        .data_dir(PathBuf::from(&cfg.data_dir))
        .relay_addrs(cfg.relay_addrs.clone())
        .advertised_addrs(cfg.advertised_addrs.clone());
    if let Some(port) = cfg.observation_port {
        builder = builder.observation_responder(port);
    }
    if !cfg.observation_addrs.is_empty() {
        builder = builder.observation_addrs(cfg.observation_addrs.clone());
    }
    builder.build().await.map_err(|e| {
        warn!(error = %e, "节点装配失败");
        format!("节点启动失败: {e}")
    })
}

/// 删除身份数据目录内的种子文件（装配层固定为 key.seed）；不存在视为已重置。
fn remove_seed(data_dir: &Path) -> Result<(), String> {
    let seed = data_dir.join("key.seed");
    match fs::remove_file(&seed) {
        Ok(()) => {
            tracing::info!(path = %seed.display(), "已删除身份种子文件");
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::info!(path = %seed.display(), "种子文件不存在，身份已视为重置");
            Ok(())
        }
        Err(e) => {
            warn!(error = %e, path = %seed.display(), "删除种子文件失败");
            Err(format!("删除身份数据失败: {e}"))
        }
    }
}

pub(super) fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
