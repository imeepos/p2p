//! discovery 命令域（F10）：邻居/地址缓存只读查询，口径同 GUI 发现页
//! （地址缓存全量 + 来源计数）。事实源为守护进程观测注册表（observe 模块），
//! 经控制通道读取，节点未启动报错退出码 1。

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::control;
use crate::error::{CliError, CliResult};
use crate::node::DEFAULT_DATA_DIR;
use crate::observe::{PeerEntry, RegistryStats};
use crate::output;
use crate::paths::Paths;

#[derive(Subcommand)]
pub enum DiscoveryCommand {
    /// 列出发现缓存（邻居与登记地址，含来源与发现时刻）
    List(ListArgs),
}

#[derive(Args)]
pub struct ListArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

/// discovery list 报告（daemon discoveryList op 同形）。
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryListReport {
    pub neighbors: Vec<PeerEntry>,
    pub stats: RegistryStats,
}

pub async fn run(cmd: DiscoveryCommand) -> CliResult<()> {
    match cmd {
        DiscoveryCommand::List(a) => list(a).await,
    }
}

async fn list(args: ListArgs) -> CliResult<()> {
    let paths = Paths::new(&args.data_dir);
    let data = control::call(&paths, json!({ "op": "discoveryList" })).await?;
    let report: DiscoveryListReport = serde_json::from_value(data)
        .map_err(|e| CliError::Runtime(format!("发现缓存解析失败: {e}")))?;
    let text = render(&report);
    output::emit(args.json, &report, &text)
}

/// 文本形态：首行来源计数汇总，随后逐邻居 key=value 行 + 地址行。
fn render(report: &DiscoveryListReport) -> String {
    if report.neighbors.is_empty() {
        return "发现缓存为空（total=0）".into();
    }
    let stats = &report.stats;
    let mut lines = vec![format!(
        "total={} mdns={} rendezvous={} manual={} connected={}",
        stats.total, stats.mdns, stats.rendezvous, stats.manual, stats.connected
    )];
    for peer in &report.neighbors {
        lines.push(format!(
            "neighbor={} source={} connected={} lastSeenMs={} firstSeenMs={}",
            peer.peer_id, peer.source, peer.connected, peer.last_seen_ms, peer.first_seen_ms
        ));
        lines.extend(peer.addrs.iter().map(|a| format!("addr={a}")));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(peers: Vec<PeerEntry>) -> DiscoveryListReport {
        DiscoveryListReport {
            stats: RegistryStats {
                total: peers.len(),
                connected: 1,
                mdns: 1,
                rendezvous: 0,
                manual: peers.len().saturating_sub(1),
            },
            neighbors: peers,
        }
    }

    #[test]
    fn text_leads_with_source_counts() {
        let peer = PeerEntry {
            peer_id: "abc".into(),
            addrs: vec!["192.168.1.5/u3400".into()],
            source: "mdns".into(),
            connected: true,
            last_seen_ms: 7,
            first_seen_ms: 3,
        };
        let text = render(&report(vec![peer]));
        for key in [
            "total=1 mdns=1 rendezvous=0 manual=0 connected=1",
            "neighbor=abc source=mdns connected=true",
            "lastSeenMs=7",
            "firstSeenMs=3",
            "addr=192.168.1.5/u3400",
        ] {
            assert!(text.contains(key), "缺 {key}: {text}");
        }
    }

    #[test]
    fn empty_cache_states_zero() {
        assert!(render(&report(vec![])).contains("total=0"));
    }
}
