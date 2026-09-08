//! acp console 子命令（INLINE-ACP-PUMP T2）：原 acp-console 独立 bin 的前台
//! 进程形态收口进 p2pctl。参数面与原 bin 等价（bootstrap/data-dir/ws-port/
//! status-port/share-link 等），装配一律调 acp-pump::console::run_console，
//! 本文件只做入参翻译，不复制实现。

use std::path::PathBuf;
use std::time::Duration;

use acp_common::consts::REATTACH_WINDOW_DEFAULT_SECS;
use acp_pump::config::parse_manual_peers;
use acp_pump::{share, ConsoleConfig};
use clap::Args;

use crate::error::{CliError, CliResult};

/// 原独立 bin 的默认数据目录（reattach 票据与 P2P 身份落这里）。
const DEFAULT_CONSOLE_DATA_DIR: &str = "./acp-console-data";

#[derive(Args)]
pub struct ConsoleArgs {
    /// 数据目录：reattach 票据落这里，P2P 身份目录 = <dir>/p2p-identity
    #[arg(long, default_value = DEFAULT_CONSOLE_DATA_DIR, value_name = "DIR")]
    pub data_dir: String,
    /// rendezvous bootstrap 地址（ip/u端口 或 ip/t端口），可多次
    #[arg(long, value_name = "ADDR")]
    pub bootstrap: Vec<String>,
    /// 关闭 mDNS 局域网发现
    #[arg(long)]
    pub no_mdns: bool,
    /// 手动登记候选 PEER@ADDR（base58 PeerId @ 底座传输地址），可多次
    #[arg(long = "peer", value_name = "PEER@ADDR")]
    pub peers: Vec<String>,
    /// 透传给 agent 桥的握手 token（也可由 WS 查询参数 atoken 逐连接指定）
    #[arg(long, value_name = "TOKEN")]
    pub agent_token: Option<String>,
    /// 本地 WS 端口（0 = 随机）
    #[arg(long, default_value_t = 0, value_name = "PORT")]
    pub ws_port: u16,
    /// status HTTP 端口（0 = 随机）
    #[arg(long, default_value_t = 0, value_name = "PORT")]
    pub status_port: u16,
    /// 断流续连窗口秒数（设计 §5 默认 90）
    #[arg(long, default_value_t = REATTACH_WINDOW_DEFAULT_SECS, value_name = "SECS")]
    pub window_secs: u64,
    /// 分享链接 dsh-acp-share://v1：启动即按链接直拨激活（设计 acp-share §7）
    #[arg(long, value_name = "URL")]
    pub share_link: Option<String>,
}

/// 前台跑泵到 ctrl_c；就绪信息以 {"kind":"ready",...} JSON 行发 stdout。
pub async fn run(args: ConsoleArgs) -> CliResult<()> {
    // 沿原 acp-console bin 语义：stderr 日志（stdout 保留给 JSON 事件行）。
    let _ = p2p_log::init(p2p_log::LogConfig::default());
    let cfg = build_config(&args)?;
    acp_pump::console::run_console(cfg)
        .await
        .map_err(CliError::Runtime)
}

/// 入参翻译：坏 --peer / 坏 --share-link 结构化报错（启动 fail-fast，不静默）。
fn build_config(args: &ConsoleArgs) -> CliResult<ConsoleConfig> {
    let manual_peers =
        parse_manual_peers(&args.peers).map_err(|e| CliError::Runtime(format!("--peer: {e}")))?;
    let share_link = match args.share_link.as_deref() {
        Some(raw) => Some(
            share::parse_share_link(raw)
                .map_err(|e| CliError::Runtime(format!("bad --share-link: {e}")))?,
        ),
        None => None,
    };
    Ok(ConsoleConfig {
        data_dir: PathBuf::from(&args.data_dir),
        bootstrap: args.bootstrap.clone(),
        mdns: !args.no_mdns,
        manual_peers,
        agent_token: args.agent_token.clone(),
        ws_port: args.ws_port,
        status_port: args.status_port,
        reattach_window: Duration::from_secs(args.window_secs),
        share_link,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> ConsoleArgs {
        ConsoleArgs {
            data_dir: "/tmp/cd".to_string(),
            bootstrap: vec!["/ip4/10.0.0.1/tcp/1".to_string()],
            no_mdns: true,
            peers: vec!["AAA@/ip4/10.0.0.2/t9".to_string()],
            agent_token: Some("tok".to_string()),
            ws_port: 8080,
            status_port: 8081,
            window_secs: 30,
            share_link: None,
        }
    }

    #[test]
    fn maps_args_into_console_config() {
        let cfg = build_config(&args()).unwrap();
        assert_eq!(cfg.data_dir, PathBuf::from("/tmp/cd"));
        assert!(!cfg.mdns);
        assert_eq!(cfg.bootstrap, vec!["/ip4/10.0.0.1/tcp/1".to_string()]);
        assert_eq!(
            cfg.manual_peers,
            vec![("AAA".to_string(), vec!["/ip4/10.0.0.2/t9".to_string()])]
        );
        assert_eq!(cfg.agent_token.as_deref(), Some("tok"));
        assert_eq!(cfg.ws_port, 8080);
        assert_eq!(cfg.status_port, 8081);
        assert_eq!(cfg.reattach_window, Duration::from_secs(30));
        assert!(cfg.share_link.is_none());
    }

    #[test]
    fn rejects_bad_manual_peer_spec() {
        let mut a = args();
        a.peers = vec!["no-at-sign".to_string()];
        let err = build_config(&a).unwrap_err();
        assert!(err.to_string().contains("--peer"), "err={err}");
    }

    #[test]
    fn rejects_bad_share_link() {
        let mut a = args();
        a.share_link = Some("https://example.com/v1?x=1".to_string());
        let err = build_config(&a).unwrap_err();
        assert!(err.to_string().contains("--share-link"), "err={err}");
    }
}
