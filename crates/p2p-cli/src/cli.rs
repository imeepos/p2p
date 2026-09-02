//! clap derive 参数（design §4 API 表面；design §12 p2p-cli 调试工具）。
//!
//! 子命令：bootstrap / node / ping / discover。所有地址为 `ip:port` 形式，
//! PeerId 为 base58(sha256(pubkey)) 的定长 32 字节编码（identity 定稿公式）。

use clap::{Parser, Subcommand};
use p2p_identity::PeerId;

/// 实验用 p2p 命令行工具：bootstrap（rendezvous+relay）/ node / ping / discover。
#[derive(Debug, Parser)]
#[command(name = "p2p-cli", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 装配 rendezvous+relay 引导节点并常驻。
    Bootstrap(BootstrapArgs),
    /// 基于 facade 起一个节点，内置 /p2p-lab/echo/1 回声 handler，常驻。
    Node(NodeArgs),
    /// 连目标节点测 RTT（对端需在跑 node 子命令并注册了 echo handler）。
    Ping(PingArgs),
    /// 列出当前发现的节点与地址。
    Discover(DiscoverArgs),
}

#[derive(Debug, clap::Args, Clone)]
pub struct BootstrapArgs {
    /// 身份数据目录（种子落盘，重启身份不变）。
    #[arg(long)]
    pub data: String,
    /// QUIC 监听地址（默认 0.0.0.0:3400/udp）。
    #[arg(long, default_value = "0.0.0.0:3400")]
    pub listen_quic: String,
    /// TCP 监听地址（默认 0.0.0.0:3401/tcp）。
    #[arg(long, default_value = "0.0.0.0:3401")]
    pub listen_tcp: String,
}

#[derive(Debug, clap::Args, Clone)]
pub struct NodeArgs {
    /// 身份数据目录（种子落盘，重启身份不变）。
    #[arg(long)]
    pub data: String,
    /// 公共引导节点（`ip/u端口` 或 `ip/t端口`）；省略则只走 mDNS。
    #[arg(long)]
    pub bootstrap: Option<String>,
    /// 显示名（仅日志可读性用）。
    #[arg(long)]
    pub name: Option<String>,
    /// QUIC 监听地址；省略则随机端口。
    #[arg(long)]
    pub listen_quic: Option<String>,
}

#[derive(Debug, clap::Args, Clone)]
pub struct PingArgs {
    /// 目标节点 PeerId（base58）。
    pub peer_id: String,
    /// 公共引导节点；用于发现目标地址。
    #[arg(long)]
    pub bootstrap: Option<String>,
    /// 等待目标被发现的最大秒数。
    #[arg(long, default_value_t = 15)]
    pub wait: u64,
}

#[derive(Debug, clap::Args, Clone)]
pub struct DiscoverArgs {
    /// 公共引导节点；省略则只走 mDNS。
    #[arg(long)]
    pub bootstrap: Option<String>,
    /// 收集发现结果的持续秒数。
    #[arg(long, default_value_t = 8)]
    pub duration: u64,
}

/// 解析 `ip:port` 为 SocketAddr；失败返回可读错误（clap 展示用）。
pub fn parse_socket_addr(s: &str) -> Result<std::net::SocketAddr, String> {
    s.parse::<std::net::SocketAddr>()
        .map_err(|e| format!("非法地址 \"{s}\": {e}"))
}

/// 解析 base58 PeerId 为 32 字节身份。
pub fn parse_peer_id(s: &str) -> Result<PeerId, String> {
    let bytes = bs58::decode(s)
        .into_vec()
        .map_err(|e| format!("PeerId 不是合法 base58: {e}"))?;
    let len = bytes.len();
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("PeerId 必须恰好 32 字节，实际 {len}"))?;
    Ok(PeerId::from_bytes(arr))
}

/// 从 `ip:port` 取端口（facade 恒绑定 0.0.0.0，IP 仅作展示）。
pub fn port_of(s: &str) -> Result<u16, String> {
    Ok(parse_socket_addr(s)?.port())
}

// ---- 上面的字符串里用了 \" 转义，下面测试单独写 ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_socket_addr_ok_and_bad() {
        let a = parse_socket_addr("127.0.0.1:3400").expect("valid addr");
        assert_eq!(a.port(), 3400);
        assert!(parse_socket_addr("no-colon").is_err());
        assert!(parse_socket_addr("1.2.3.4:notaport").is_err());
    }

    #[test]
    fn parse_peer_id_roundtrip() {
        let kp = p2p_identity::Keypair::generate();
        let s = kp.peer_id().to_string();
        let parsed = parse_peer_id(&s).expect("parse own peer id");
        assert_eq!(parsed, kp.peer_id());
    }

    #[test]
    fn parse_peer_id_rejects_wrong_len() {
        assert!(parse_peer_id("abc").is_err(), "base58 but wrong bytes");
        // 恰好 31 字节的合法 base58
        let short = bs58::encode([7u8; 31]).into_string();
        assert!(parse_peer_id(&short).is_err(), "31 bytes must fail");
    }

    #[test]
    fn parse_peer_id_rejects_non_base58() {
        assert!(parse_peer_id("!!!not-base58!!!").is_err());
    }

    #[test]
    fn port_of_extracts_port() {
        assert_eq!(port_of("0.0.0.0:3400").unwrap(), 3400);
        assert_eq!(port_of("[::1]:4400").unwrap(), 4400);
    }
}
