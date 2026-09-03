//! chat 槽位（契约 v7 §12，T32）：p2p-chat 实例装配/卸载与访问。
//!
//! 与 node 同生命周期：node_start 时装配（依赖 node 已启动），node_stop/reset 时卸载；
//! 未装配时 chat 命令一律可读中文 Err（禁止静默）。

use std::path::PathBuf;
use std::sync::Arc;

use p2p::Node;
use p2p_chat::{Chat, ChatEvent};
use tokio::sync::{broadcast, Mutex};
use tracing::warn;

/// p2p-chat 槽位：持有可选实例，装配在 node_start、卸载在 node_stop/reset。
pub struct ChatSlot {
    app_data_dir: PathBuf,
    slot: Mutex<Option<Arc<Chat>>>,
}

impl ChatSlot {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            app_data_dir,
            slot: Mutex::new(None),
        }
    }

    /// 节点启动后装配（data_dir = app_data_dir，crate 内部 join "chat"）；
    /// 返回 chat 事件接收端（命令层转发）。失败留告警并回抛中文 Err。
    pub async fn install(&self, node: Arc<Node>) -> Result<broadcast::Receiver<ChatEvent>, String> {
        let chat = Chat::new(node, self.app_data_dir.clone()).map_err(|e| {
            warn!(error = %e, "聊天模块装配失败");
            format!("聊天模块装配失败: {e}")
        })?;
        let rx = chat.events();
        *self.slot.lock().await = Some(Arc::new(chat));
        Ok(rx)
    }

    /// 节点停止时卸载（Arc 释放；outbox 任务随节点事件通道关闭自然退出）。
    pub async fn uninstall(&self) {
        *self.slot.lock().await = None;
    }

    /// 取聊天实例；未装配（节点未启动）返回可读中文 Err。
    pub async fn get(&self) -> Result<Arc<Chat>, String> {
        self.slot
            .lock()
            .await
            .clone()
            .ok_or_else(|| "节点未运行，聊天功能不可用".to_string())
    }
}
