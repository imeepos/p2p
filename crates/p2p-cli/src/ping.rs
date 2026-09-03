//! ping 子命令：经 bootstrap/mDNS 发现目标后，走 echo 协议测 RTT。

use std::time::{Duration, Instant};

use p2p::{Node, NodeBuilder, NodeEvent, ProtocolId};
use tokio::sync::broadcast;

use crate::cli::{parse_peer_id, PingArgs};
use crate::echo::{ECHO_PROTOCOL, PING_PAYLOAD};

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
    let request_timeout = Duration::from_secs(args.request_timeout);
    let request = node.request(target, id, PING_PAYLOAD.to_vec(), request_timeout);
    tokio::pin!(request);
    // 请求期间同步打印 DialHop 逐跳事件（直连/打洞/中继），路径随结果一起留档
    let mut reply = None;
    while reply.is_none() {
        tokio::select! {
            r = &mut request => reply = Some(r),
            hop = next_hop(&mut events) => {
                if let Some(line) = hop {
                    println!("{line}");
                } else {
                    break;
                }
            }
        }
    }
    let reply = reply
        .expect("循环仅以应答结束")
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

/// 读下一条 DialHop 事件；通道关闭返回 None（事件流随节点存续，不闭合）。
async fn next_hop(events: &mut broadcast::Receiver<NodeEvent>) -> Option<String> {
    loop {
        match events.recv().await {
            Ok(NodeEvent::DialHop {
                peer,
                hop,
                ok,
                detail,
            }) => {
                return Some(format!("hop {hop:?} ok={ok} detail={detail} ({peer})"));
            }
            Ok(_) => continue,
            Err(broadcast::error::RecvError::Closed) => return None,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
        }
    }
}

/// 一次性临时数据目录（ping 无需持久身份，避免污染 cwd）。
fn tmp_data_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{tag}-{}", std::process::id()))
}

/// 装配 ping 节点：mdns on（局域网直连）+ 可选 bootstrap（跨网发现）+
/// 观测反射（E5：注册可路由地址，地址卫生过滤下无观测即不可被发现）。
async fn build_node(args: &PingArgs) -> Result<Node, Box<dyn std::error::Error>> {
    let mut builder = NodeBuilder::new()
        .mdns(!args.no_mdns)
        .data_dir(tmp_data_dir("p2p-ping"));
    if !args.bootstrap.is_empty() {
        builder = builder.bootstrap(args.bootstrap.clone());
    }
    if !args.relay.is_empty() {
        builder = builder.relay_addrs(args.relay.clone());
    }
    if !args.observation.is_empty() {
        builder = builder.observation_addrs(args.observation.clone());
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
