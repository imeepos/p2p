//! discover 子命令：采集一段窗口内的 PeerDiscovered 事件，列出节点与地址。

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use p2p::{Node, NodeBuilder, NodeEvent};

use crate::cli::DiscoverArgs;

/// 采集某段时间内的发现事件，按 PeerId 汇总去重后打印。
pub async fn run(args: DiscoverArgs) -> Result<(), String> {
    let node = build_node(&args)
        .await
        .map_err(|e| format!("装配节点失败: {e}"))?;
    println!(
        "collecting discovered peers for {}s (local peer_id={})...",
        args.duration,
        node.local_peer_id()
    );

    let mut peers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut events = node.events();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(args.duration);
    loop {
        let budget = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(budget, events.recv()).await {
            Ok(Ok(NodeEvent::PeerDiscovered { peer, addrs })) => {
                let set = peers.entry(peer.to_string()).or_default();
                for a in addrs {
                    set.insert(a);
                }
            }
            Ok(Ok(_)) => continue,
            Ok(Err(_)) => continue,
            Err(_) => break, // 窗口结束
        }
    }

    if peers.is_empty() {
        println!("no peers discovered in {}s", args.duration);
        return Ok(());
    }
    for (peer, addrs) in &peers {
        println!("{peer}");
        for a in addrs {
            println!("  {a}");
        }
    }
    println!("found {} peer(s)", peers.len());
    Ok(())
}

/// 一次性临时数据目录（discover 无需持久身份，避免污染 cwd）。
fn tmp_data_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{tag}-{}", std::process::id()))
}

/// 采集节点：mdns on + 可选 bootstrap。
async fn build_node(args: &DiscoverArgs) -> Result<Node, Box<dyn std::error::Error>> {
    let mut builder = NodeBuilder::new()
        .mdns(!args.no_mdns)
        .data_dir(tmp_data_dir("p2p-discover"));
    if !args.bootstrap.is_empty() {
        builder = builder.bootstrap(args.bootstrap.clone());
    }
    Ok(builder.build().await?)
}
