//! acp-console 入口（薄壳）：CLI 入参翻译成 ConsoleConfig 后调 acp-pump 装配。
//! 实现一律在 acp-pump（INLINE-ACP-PUMP T1）；本文件将在 T5 随目录删除。

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::time::Duration;

use acp_pump::config::parse_manual_peers;
use acp_pump::console::run_console;
use acp_pump::{share, ConsoleConfig};
use clap::Parser;

#[derive(Parser)]
#[command(
    name = "acp-console",
    about = "ACP over P2P 操作者侧伴生进程：本地 WS(127.0.0.1+token) ⇄ P2P 流哑泵 + 节点发现"
)]
struct Args {
    /// 数据目录：reattach 票据落这里，P2P 身份目录 = <dir>/p2p-identity
    #[arg(long, default_value = "./acp-console-data", value_name = "DIR")]
    data_dir: PathBuf,
    /// rendezvous bootstrap 地址（ip/u端口 或 ip/t端口），可多次
    #[arg(long, value_name = "ADDR")]
    bootstrap: Vec<String>,
    /// 关闭 mDNS 局域网发现
    #[arg(long)]
    no_mdns: bool,
    /// 手动登记候选 PEER@ADDR（base58 PeerId @ 底座传输地址），可多次
    #[arg(long = "peer", value_name = "PEER@ADDR")]
    peers: Vec<String>,
    /// 透传给 agent 桥的握手 token（也可由 WS 查询参数 atoken 逐连接指定）
    #[arg(long, value_name = "TOKEN")]
    agent_token: Option<String>,
    /// 本地 WS 端口（0 = 随机）
    #[arg(long, default_value_t = 0, value_name = "PORT")]
    ws_port: u16,
    /// status HTTP 端口（0 = 随机）
    #[arg(long, default_value_t = 0, value_name = "PORT")]
    status_port: u16,
    /// 断流续连窗口秒数（设计 §5 默认 90）
    #[arg(long, default_value_t = acp_common::consts::REATTACH_WINDOW_DEFAULT_SECS, value_name = "SECS")]
    window_secs: u64,
    /// 分享链接 dsh-acp-share://v1：启动即按链接直拨激活（设计 acp-share §7）
    #[arg(long, value_name = "URL")]
    share_link: Option<String>,
}

fn main() -> Result<(), String> {
    let args = Args::parse();
    let _ = p2p_log::init(p2p_log::LogConfig::default());
    let manual_peers = parse_manual_peers(&args.peers)?;
    let share_link = match args.share_link.as_deref() {
        Some(raw) => {
            Some(share::parse_share_link(raw).map_err(|e| format!("bad --share-link: {e}"))?)
        }
        None => None,
    };
    // CLI 入参统一收敛进 ConsoleConfig（库面装配契约），main 只做翻译不做业务。
    let cfg = ConsoleConfig {
        data_dir: args.data_dir,
        bootstrap: args.bootstrap,
        mdns: !args.no_mdns,
        manual_peers,
        agent_token: args.agent_token,
        ws_port: args.ws_port,
        status_port: args.status_port,
        reattach_window: Duration::from_secs(args.window_secs),
        share_link,
    };
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("tokio runtime: {e}"))?
        .block_on(run_console(cfg))
}
