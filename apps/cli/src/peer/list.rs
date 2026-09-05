//! peer list（F10）：地址簿 + 在线态只读查询。事实源为守护进程观测注册表
//! （observe 模块），本模块只做控制通道取数与文本/JSON 双形态渲染。

use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::control;
use crate::error::CliResult;
use crate::node::DEFAULT_DATA_DIR;
use crate::observe::PeerEntry;
use crate::output;
use crate::paths::Paths;

#[derive(Args)]
pub struct ListArgs {
    /// 输出结构化 JSON
    #[arg(long)]
    json: bool,
    /// CLI 数据目录
    #[arg(long, default_value = DEFAULT_DATA_DIR)]
    data_dir: String,
}

/// peer list 报告（daemon peerList op 同形）。
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerListReport {
    pub peers: Vec<PeerEntry>,
    pub total: usize,
    pub connected: usize,
}

pub async fn run(args: ListArgs) -> CliResult<()> {
    let paths = Paths::new(&args.data_dir);
    let data = control::call(&paths, json!({ "op": "peerList" })).await?;
    let report: PeerListReport =
        serde_json::from_value(data).map_err(|e| crate::error::CliError::Runtime(format!("地址簿解析失败: {e}")))?;
    let text = render(&report);
    output::emit(args.json, &report, &text)
}

/// 文本形态：首行汇总，随后每对端一行 key=value，地址行紧随其后。
fn render(report: &PeerListReport) -> String {
    if report.peers.is_empty() {
        return "地址簿为空（total=0 connected=0）".into();
    }
    let mut lines = vec![format!("total={} connected={}", report.total, report.connected)];
    for peer in &report.peers {
        lines.push(format!(
            "peer={} connected={} source={} lastSeenMs={} firstSeenMs={}",
            peer.peer_id, peer.connected, peer.source, peer.last_seen_ms, peer.first_seen_ms
        ));
        lines.extend(peer.addrs.iter().map(|a| format!("addr={a}")));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_peer() -> String {
        bs58::encode([7u8; 32]).into_string()
    }

    fn entry(source: &str, connected: bool, addr: &str) -> PeerEntry {
        PeerEntry {
            peer_id: sample_peer(),
            addrs: vec![addr.into()],
            source: source.into(),
            connected,
            last_seen_ms: 123,
            first_seen_ms: 100,
        }
    }

    #[test]
    fn text_is_key_value_greppable() {
        let report = PeerListReport {
            peers: vec![entry("mdns", true, "192.168.1.5/u3400")],
            total: 1,
            connected: 1,
        };
        let text = render(&report);
        for key in [
            "total=1",
            "connected=1",
            &format!("peer={}", sample_peer()),
            "connected=true",
            "source=mdns",
            "addr=192.168.1.5/u3400",
        ] {
            assert!(text.contains(key), "缺 {key}: {text}");
        }
    }

    #[test]
    fn empty_book_states_zero_counts() {
        let report = PeerListReport {
            peers: vec![],
            total: 0,
            connected: 0,
        };
        assert!(render(&report).contains("total=0 connected=0"));
    }
}
