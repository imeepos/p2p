//! vdrive serve/mount 共用：节点装配、信号等待、JSON 行就绪/收口输出。

use std::path::PathBuf;

use clap::Args;
use p2p::Node;

use crate::error::{CliError, CliResult};

/// 两端相同的节点参数（形态对齐 tunnel 域）。
#[derive(Args)]
pub struct NodeArgs {
    /// 节点身份数据目录
    #[arg(long, default_value = crate::node::DEFAULT_DATA_DIR, value_name = "DIR")]
    pub data_dir: String,
    /// QUIC 监听端口（0 = 随机）
    #[arg(long, default_value_t = 0, value_name = "PORT")]
    pub quic_port: u16,
    /// TCP 监听端口（0 = 随机）
    #[arg(long, default_value_t = 0, value_name = "PORT")]
    pub tcp_port: u16,
    /// 关闭 mDNS 局域网发现
    #[arg(long)]
    pub no_mdns: bool,
    /// rendezvous bootstrap 地址（ip/u端口 或 ip/t端口），可多次
    #[arg(long, value_name = "ADDR")]
    pub bootstrap: Vec<String>,
}

/// 节点装配（daemon/tunnel 同款缝）。
pub async fn build_node(args: &NodeArgs) -> CliResult<Node> {
    let mut builder = Node::builder()
        .quic_port(args.quic_port)
        .tcp_port(args.tcp_port)
        .mdns(!args.no_mdns)
        .data_dir(PathBuf::from(&args.data_dir));
    if !args.bootstrap.is_empty() {
        builder = builder.bootstrap(args.bootstrap.clone());
    }
    builder
        .build()
        .await
        .map_err(|e| CliError::Runtime(format!("节点启动失败: {e}")))
}

/// SIGINT/SIGTERM 等待。
pub async fn wait_signal() {
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("装 SIGTERM");
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .expect("装 SIGINT");
    tokio::select! {
        _ = sigterm.recv() => {}
        _ = sigint.recv() => {}
    }
}

/// PeerId 解析（base58 32 字节，与 peer/tunnel 域同规则）。
pub fn parse_peer(raw: &str) -> CliResult<p2p_identity::PeerId> {
    let bytes = bs58::decode(raw.trim())
        .into_vec()
        .map_err(|e| CliError::Runtime(format!("--peer 非 base58: {e}")))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| CliError::Runtime(format!("--peer 长度非法（须 32 字节）: {raw}")))?;
    Ok(p2p_identity::PeerId::from_bytes(arr))
}

pub fn emit(line: serde_json::Value) {
    println!("{line}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_peer_rejects_bad_input() {
        assert!(parse_peer("not-base58!!").is_err());
        assert!(parse_peer("").is_err());
    }

    #[test]
    fn parse_peer_accepts_32_byte_base58() {
        let raw = bs58::encode([7u8; 32]).into_string();
        assert!(parse_peer(&raw).is_ok());
    }
}
