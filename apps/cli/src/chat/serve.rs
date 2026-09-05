//! chat serve：常驻聊天节点（E2E/守护支撑命令）。
//!
//! 数据面与 GUI node_start+chat 完全一致（Chat::new：存储/入站 handler/outbox）；
//! 传输面默认不接 bootstrap、mdns 默认关（--mdns 开启），E2E 隔离必需。
//! stdout 只输出一行就绪信息（--json 为单行紧凑 JSON），运行期事件镜像到 stderr。

use std::path::{Path, PathBuf};
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
    /// QUIC 监听端口（缺省沿用上次记忆端口，首启随机；显式指定优先并覆盖记忆）
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

pub async fn run(args: ServeArgs) -> CliResult<()> {
    // 身份进程互斥（D6 裁决）：同数据目录不支持多程序并行，被占即快速失败。
    let _identity = p2p_chat::try_lock_identity(Path::new(&args.data_dir)).map_err(runtime_err)?;
    let data_dir = PathBuf::from(&args.data_dir);
    // 端口记忆（F1）：显式 --quic-port > 上次记忆端口 > 随机；记忆端口绑定失败
    // （上次进程未退/端口被抢）回退随机并留 stderr 观测，禁止静默漂移。
    let explicit = args.quic_port;
    let remembered = p2p_chat::load_serve_port(&data_dir);
    let requested = explicit.or(remembered).unwrap_or(0);
    let built = context::builder(&args.data_dir, requested, args.mdns).build().await;
    let node = match built {
        Ok(node) => node,
        Err(e) if explicit.is_none() && remembered.is_some() => {
            eprintln!(
                "chat serve: 记忆端口 {requested} 绑定失败（{e}），回退随机端口",
            );
            context::builder(&args.data_dir, 0, args.mdns)
                .build()
                .await
                .map_err(|e| {
                    CliError::Runtime(format!("节点装配失败（data-dir={}）: {e}", args.data_dir))
                })?
        }
        Err(e) => {
            return Err(CliError::Runtime(format!(
                "节点装配失败（data-dir={}）: {e}",
                args.data_dir
            )));
        }
    };
    let node = Arc::new(node);
    let chat = Chat::new(node.clone(), PathBuf::from(&args.data_dir)).map_err(runtime_err)?;
    chat.publish_advertised().map_err(runtime_err)?;
    let info = ServeInfo {
        peer_id: node.local_peer_id().to_string(),
        listen_addrs: node.listen_addrs(),
        data_dir: args.data_dir.clone(),
    };
    spawn_event_echo(chat.events());
    // 绑定成功即刷新端口记忆（显式/随机端口同样落盘，重启均可沿用）。
    match quic_port_of(&info.listen_addrs) {
        Some(port) => {
            if let Err(e) = p2p_chat::save_serve_port(&data_dir, port) {
                eprintln!("chat serve: 端口记忆写入失败: {e}");
            }
        }
        None => eprintln!("chat serve: 无 QUIC 监听地址，端口记忆未更新"),
    }
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

/// 从监听地址提取 QUIC 端口（ip/u端口 形态）；无 QUIC 监听 → None。
fn quic_port_of(listen_addrs: &[String]) -> Option<u16> {
    listen_addrs
        .iter()
        .find_map(|a| a.rsplit("/u").next()?.parse::<u16>().ok())
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
    fn quic_port_extracts_u_suffix_only() {
        let addrs = vec![
            "127.0.0.1/u60645".to_string(),
            "127.0.0.1/t61793".to_string(),
        ];
        assert_eq!(quic_port_of(&addrs), Some(60645));
        assert_eq!(quic_port_of(&["127.0.0.1/t61793".to_string()]), None);
    }

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
