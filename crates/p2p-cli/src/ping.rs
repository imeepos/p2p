//! ping 子命令：经 bootstrap/mDNS 发现目标后，走 echo 协议测 RTT。

use std::time::{Duration, Instant};

use p2p::{Node, NodeBuilder, NodeEvent, ProtocolId};
use tokio::sync::broadcast;

use crate::cli::{parse_peer_id, PingArgs};
use crate::echo::{ECHO_PROTOCOL, PING_PAYLOAD};

/// echo request 的超时：含逐地址拨号回退，多接口地址簿需较长预算。
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// 测目标 RTT：等待被发现 -> echo request -> 计算往返耗时。
pub async fn run(args: PingArgs) -> Result<(), String> {
    let target = parse_peer_id(&args.peer_id)?;
    let node = build_node(&args)
        .await
        .map_err(|e| format!("装配节点失败: {e}"))?;

    println!(
        "pinging {target} (awaiting discovery, up to {}s)",
        args.wait
    );
    let mut events = node.events();
    wait_discovered(&mut events, target, Duration::from_secs(args.wait))
        .await
        .map_err(|why| format!("目标未在 {}s 内被发现: {why}", args.wait))?;

    let started = Instant::now();
    let id = ProtocolId::new(ECHO_PROTOCOL).expect("built-in echo id is valid");
    let reply = node
        .request(target, id, PING_PAYLOAD.to_vec(), REQUEST_TIMEOUT)
        .await
        .map_err(|e| format!("echo 请求失败: {e}"))?;
    let rtt = started.elapsed();
    if reply != PING_PAYLOAD {
        return Err(format!("echo 应答与请求不符（实得 {} 字节）", reply.len()));
    }
    println!(
        "pong from {target}: rtt={rtt:?} reply={} bytes",
        reply.len()
    );
    Ok(())
}

/// 一次性临时数据目录（ping 无需持久身份，避免污染 cwd）。
fn tmp_data_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{tag}-{}", std::process::id()))
}

/// 装配 ping 节点：mdns on（局域网直连）+ 可选 bootstrap（跨网发现）。
async fn build_node(args: &PingArgs) -> Result<Node, Box<dyn std::error::Error>> {
    let mut builder = NodeBuilder::new()
        .mdns(!args.no_mdns)
        .data_dir(tmp_data_dir("p2p-ping"));
    if let Some(addr) = &args.bootstrap {
        builder = builder.bootstrap(vec![addr.clone()]);
    }
    Ok(builder.build().await?)
}

/// 轮询发现事件直到目标出现；超时返回明确错误。
async fn wait_discovered(
    events: &mut broadcast::Receiver<NodeEvent>,
    target: p2p::PeerId,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let budget = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(budget, events.recv()).await {
            Ok(Ok(NodeEvent::PeerDiscovered { peer, .. })) if peer == target => return Ok(()),
            Ok(Ok(_)) | Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
            Ok(Err(broadcast::error::RecvError::Closed)) => return Err("事件通道关闭".into()),
            Err(_) => return Err("发现窗口超时（目标离线或 bootstrap 不可达）".into()),
        }
    }
}
