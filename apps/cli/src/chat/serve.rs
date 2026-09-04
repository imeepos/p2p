//! chat serve：常驻聊天节点（E2E/守护支撑命令）。
//!
//! 数据面与 GUI node_start+chat 完全一致（Chat::new：存储/入站 handler/outbox）；
//! 传输面默认不接 bootstrap、mdns 默认关（--mdns 开启），E2E 隔离必需。
//! stdout 只输出一行就绪信息（--json 为单行紧凑 JSON），运行期事件镜像到 stderr。

use std::path::PathBuf;
use std::sync::Arc;

use clap::Args;
use p2p_chat::Chat;
use serde::Serialize;

use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;

use super::{context, emit, runtime_err};

#[derive(Args)]
pub struct ServeArgs {
    /// 数据目录（身份与聊天库同根）
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
    /// QUIC 监听端口（默认随机）
    #[arg(long)]
    quic_port: Option<u16>,
    /// 启用 mDNS 局域网发现（默认关）
    #[arg(long)]
    mdns: bool,
    /// 输出单行紧凑 JSON
    #[arg(long)]
    json: bool,
}

/// 就绪信息；listenAddrs 为 ip/u端口 / ip/t端口，可直接作 friend add --addr。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ServeInfo {
    peer_id: String,
    listen_addrs: Vec<String>,
    data_dir: String,
}

pub async fn run(args: ServeArgs) -> CliResult {
    let node = context::builder(&args.data_dir, args.quic_port.unwrap_or(0), args.mdns)
        .build()
        .await
        .map_err(|e| {
            CliError::Runtime(format!("节点装配失败（data-dir={}）: {e}", args.data_dir))
        })?;
    let node = Arc::new(node);
    let chat = Chat::new(node.clone(), PathBuf::from(&args.data_dir)).map_err(runtime_err)?;
    let info = ServeInfo {
        peer_id: node.local_peer_id().to_string(),
        listen_addrs: node.listen_addrs(),
        data_dir: args.data_dir.clone(),
    };
    spawn_event_echo(chat.events());
    let text = format!(
        "chat 节点就绪 peer={} listen={}",
        info.peer_id,
        info.listen_addrs.join(" ")
    );
    emit(args.json, &info, &text)?;
    wait_stop().await?;
    node.shutdown();
    Ok(())
}

/// chat 事件镜像到 stderr（chat_message/chat_status 单行 JSON）：失败路径可观测，
/// 且不污染 stdout 的机器可读就绪信息。
fn spawn_event_echo(mut rx: tokio::sync::broadcast::Receiver<p2p_chat::ChatEvent>) {
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => match serde_json::to_string(&event) {
                    Ok(line) => eprintln!("{line}"),
                    Err(e) => eprintln!("chat 事件序列化失败: {e}"),
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skip)) => {
                    eprintln!("chat 事件通道滞后，丢弃 {skip} 条");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

#[cfg(unix)]
async fn wait_stop() -> Result<(), CliError> {
    use tokio::signal::unix::{signal, SignalKind};
    let mut term = signal(SignalKind::terminate())
        .map_err(|e| CliError::Runtime(format!("注册 SIGTERM 失败: {e}")))?;
    tokio::select! {
        r = tokio::signal::ctrl_c() => {
            r.map_err(|e| CliError::Runtime(format!("等待 Ctrl-C 失败: {e}")))?
        }
        _ = term.recv() => {}
    }
    Ok(())
}

#[cfg(not(unix))]
async fn wait_stop() -> Result<(), CliError> {
    tokio::signal::ctrl_c()
        .await
        .map_err(|e| CliError::Runtime(format!("等待 Ctrl-C 失败: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serve_info_json_uses_camel_case() {
        let info = ServeInfo {
            peer_id: "PEER".into(),
            listen_addrs: vec!["127.0.0.1/u1".into()],
            data_dir: "./d".into(),
        };
        let v = serde_json::to_value(&info).unwrap();
        assert_eq!(v["peerId"], serde_json::json!("PEER"));
        assert_eq!(v["listenAddrs"][0], serde_json::json!("127.0.0.1/u1"));
        assert_eq!(v["dataDir"], serde_json::json!("./d"));
    }
}
