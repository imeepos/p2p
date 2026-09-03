//! clap derive 参数（design §4 API 表面；design §12 p2p-cli 调试工具）。
//!
//! 子命令：bootstrap / node / ping / discover。所有地址为 `ip:port` 形式，
//! PeerId 为 base58(sha256(pubkey)) 的定长 32 字节编码（identity 定稿公式）。

use clap::{Parser, Subcommand};
use p2p_identity::PeerId;

/// 实验用 p2p 命令行工具：bootstrap（rendezvous+relay）/ node / ping / discover / metrics。
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
    /// 中继指标观测（E8-M2）：relay-only 节点，stdout 周期打印 relay_ 前缀 key=value。
    Metrics(MetricsArgs),
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
    /// 观测反射口（UDP，供节点学习 NAT 映射地址；默认 3402/udp）。
    #[arg(long, default_value_t = 3402)]
    pub observation_port: u16,
    /// 允许全 loopback/link-local 地址注册（同机/内网试验用）；公共部署勿开（E5）。
    #[arg(long, default_value_t = false)]
    pub allow_private: bool,
}

#[derive(Debug, clap::Args, Clone)]
pub struct NodeArgs {
    /// 身份数据目录（种子落盘，重启身份不变）。
    #[arg(long)]
    pub data: String,
    /// 公共引导节点（`ip/u端口` 或 `ip/t端口`，可多次传入注册双引导面）；省略则只走 mDNS。
    #[arg(long, value_name = "ADDR", action = clap::ArgAction::Append)]
    pub bootstrap: Vec<String>,
    /// 中继服务地址（`ip/u端口`，可多次传入）：接线 relay_addrs 启用打洞信令与中继兜底。
    #[arg(long, value_name = "ADDR", action = clap::ArgAction::Append)]
    pub relay: Vec<String>,
    /// 显示名（仅日志可读性用）。
    #[arg(long)]
    pub name: Option<String>,
    /// QUIC 监听地址；省略则随机端口。
    #[arg(long)]
    pub listen_quic: Option<String>,
    /// 观测口地址（ip:port，可多次传入），启动时学习自身公网映射地址。
    #[arg(long = "observation", value_name = "HOST:PORT", action = clap::ArgAction::Append)]
    pub observation: Vec<String>,
    /// 关闭 mDNS 局域网发现（跨网实验只需 rendezvous 时使用）。
    #[arg(long)]
    pub no_mdns: bool,
}

#[derive(Debug, clap::Args, Clone)]
pub struct PingArgs {
    /// 目标节点 PeerId（base58）。
    pub peer_id: String,
    /// 公共引导节点；用于发现目标地址（可多次传入，任一引导面可用即可）。
    #[arg(long, value_name = "ADDR", action = clap::ArgAction::Append)]
    pub bootstrap: Vec<String>,
    /// 中继服务地址（`ip/u端口`，可多次传入）：接线后直连失败走打洞/中继兜底。
    #[arg(long, value_name = "ADDR", action = clap::ArgAction::Append)]
    pub relay: Vec<String>,
    /// 等待目标被发现的最大秒数。
    #[arg(long, default_value_t = 15)]
    pub wait: u64,
    /// echo request 超时秒数（E5：云端安全组黑洞直连会吃满拨号预算，可调大）。
    #[arg(long, default_value_t = 20)]
    pub request_timeout: u64,
    /// 观测口地址（ip:port，可多次传入）：注册可路由地址，地址卫生过滤下无观测不可被发现。
    #[arg(long = "observation", value_name = "HOST:PORT", action = clap::ArgAction::Append)]
    pub observation: Vec<String>,
    /// 关闭 mDNS 局域网发现（跨网实验只需 rendezvous 时使用）。
    #[arg(long)]
    pub no_mdns: bool,
}

#[derive(Debug, clap::Args, Clone)]
pub struct DiscoverArgs {
    /// 公共引导节点；省略则只走 mDNS（可多次传入）。
    #[arg(long, value_name = "ADDR", action = clap::ArgAction::Append)]
    pub bootstrap: Vec<String>,
    /// 收集发现结果的持续秒数。
    #[arg(long, default_value_t = 8)]
    pub duration: u64,
    /// 关闭 mDNS 局域网发现（跨网实验只需 rendezvous 时使用）。
    #[arg(long)]
    pub no_mdns: bool,
}

#[derive(Debug, clap::Args, Clone)]
pub struct MetricsArgs {
    /// 身份数据目录（种子落盘，重启身份不变）。
    #[arg(long)]
    pub data: String,
    /// relay QUIC 监听地址（部署约定 = bootstrap listen-quic +3 端口）。
    #[arg(long, default_value = "0.0.0.0:3403")]
    pub listen_quic: String,
    /// relay TCP 监听地址（部署约定 = bootstrap listen-tcp +3 端口）。
    #[arg(long, default_value = "0.0.0.0:3404")]
    pub listen_tcp: String,
    /// 快照打印周期秒数。
    #[arg(long, default_value_t = 10)]
    pub interval: u64,
    /// 自动退出秒数；0 = 常驻直到 ctrl-c。
    #[arg(long, default_value_t = 0)]
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

    #[test]
    fn bootstrap_default_observation_port_is_3402() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "p2p-cli",
            "bootstrap",
            "--data",
            "d",
            "--listen-quic",
            "0.0.0.0:3400",
            "--listen-tcp",
            "0.0.0.0:3401",
        ])
        .expect("parse bootstrap defaults");
        let Command::Bootstrap(args) = cli.command else {
            panic!("expected bootstrap command");
        };
        assert_eq!(args.observation_port, 3402);
    }

    #[test]
    fn bootstrap_accepts_custom_observation_port() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "p2p-cli",
            "bootstrap",
            "--data",
            "d",
            "--observation-port",
            "4402",
        ])
        .expect("parse bootstrap observation-port");
        let Command::Bootstrap(args) = cli.command else {
            panic!("expected bootstrap command");
        };
        assert_eq!(args.observation_port, 4402);
    }

    #[test]
    fn node_observation_append_multiple() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "p2p-cli",
            "node",
            "--data",
            "d",
            "--observation",
            "1.2.3.4:3402",
            "--observation",
            "5.6.7.8:3402",
        ])
        .expect("parse node observation addrs");
        let Command::Node(args) = cli.command else {
            panic!("expected node command");
        };
        assert_eq!(args.observation.len(), 2);
        assert_eq!(args.observation[0], "1.2.3.4:3402");
        assert_eq!(args.observation[1], "5.6.7.8:3402");
    }

    #[test]
    fn no_mdns_flag_parses() {
        use clap::Parser;
        let cli = Cli::try_parse_from([
            "p2p-cli",
            "node",
            "--data",
            "d",
            "--no-mdns",
            "--observation",
            "1.2.3.4:3402",
        ])
        .expect("parse node no-mdns");
        let Command::Node(args) = cli.command else {
            panic!("expected node command");
        };
        assert!(args.no_mdns);
        assert_eq!(args.observation.len(), 1);
    }

    #[test]
    fn node_observation_default_empty() {
        use clap::Parser;
        let cli = Cli::try_parse_from(["p2p-cli", "node", "--data", "d"])
            .expect("parse node no observation");
        let Command::Node(args) = cli.command else {
            panic!("expected node command");
        };
        assert!(args.observation.is_empty());
    }
}
