//! chat 命令公共装配：--data-dir 一处收敛。节点身份（key.seed）与聊天库
//! （<data-dir>/chat）同根；与 GUI 指向同一目录即看到同一份好友与历史（R2）。
//!
//! 一次性命令的节点保持最小装配：随机端口、不接 bootstrap、mdns 默认关——
//! 投递只依赖好友簿地址（Chat::new 内 rearm_friend_addrs 回填），E2E 可静态隔离。

use std::path::PathBuf;

use p2p::{Node, NodeBuilder};
use p2p_chat::Chat;

use crate::error::CliError;

pub struct ChatContext {
    pub chat: Chat,
}

/// serve 与一次性命令共用的节点装配参数（quic 端口/mdns 开关可调）。
pub(crate) fn builder(data_dir: &str, quic_port: u16, mdns: bool) -> NodeBuilder {
    Node::builder()
        .quic_port(quic_port)
        .tcp_port(0)
        .mdns(mdns)
        .data_dir(PathBuf::from(data_dir))
}

/// 装配一次性命令上下文：节点 + chat 门面（存储/入站 handler/outbox 全在门面内）。
pub async fn open(data_dir: &str) -> Result<ChatContext, CliError> {
    let node = builder(data_dir, 0, false)
        .build()
        .await
        .map_err(|e| CliError::Runtime(format!("节点装配失败（data-dir={data_dir}）: {e}")))?;
    let chat = Chat::new(std::sync::Arc::new(node), PathBuf::from(data_dir))
        .map_err(|e| CliError::Runtime(format!("聊天模块装配失败: {e}")))?;
    Ok(ChatContext { chat })
}