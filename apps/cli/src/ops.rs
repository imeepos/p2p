//! 守护进程侧操作实现：语义对齐 GUI AppState（state.rs / state/peers.rs）。
//! dial = 登记地址 + 连接；connect = 地址簿直连；disconnect 幂等；ping 走 echo 协议。

use std::time::{Duration, Instant};

use p2p::{Node, NodeEvent, PeerId, ProtocolId};
use serde_json::{json, Value};

use crate::types::{DialHopJson, DialReport, HopKind, MetricsJson, NodeStatus, PingOutcome};

/// 解析 base58 PeerId（32 字节定长，规则同 GUI proto::parse_peer_id）。
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

/// 解析 "<peer_id>@<addr>"（契约 §6）；addr 为 ip/u端口 或 ip/t端口。
pub fn parse_target(target: &str) -> Result<(PeerId, String), String> {
    let (peer_str, addr) = target.split_once('@').ok_or_else(|| {
        format!("target 缺少 '@' 分隔符，应为 <peer_id>@<addr>，实得 \"{target}\"")
    })?;
    let peer = parse_peer_id(peer_str)?;
    validate_addr(addr)?;
    Ok((peer, addr.to_string()))
}

fn validate_addr(addr: &str) -> Result<(), String> {
    let bad = || format!("非法地址 \"{addr}\"，应为 ip/u端口（QUIC）或 ip/t端口（TCP）");
    let (ip_str, tail) = addr.split_once('/').ok_or_else(bad)?;
    ip_str.parse::<std::net::IpAddr>().map_err(|_| bad())?;
    let mut chars = tail.chars();
    let kind = chars.next().ok_or_else(bad)?;
    if kind != 'u' && kind != 't' {
        return Err(bad());
    }
    let port: u16 = chars.as_str().parse().map_err(|_| bad())?;
    if port == 0 {
        return Err("端口不能为 0".into());
    }
    Ok(())
}

/// 状态快照（含生效配置的 JSON 形态）。
pub fn status_json(node: &Node, config: &crate::types::GuiConfig, started_at: Instant, started_at_ms: u64) -> Value {
    let status = NodeStatus {
        running: true,
        peer_id: Some(node.local_peer_id().to_string()),
        listen_addrs: node.listen_addrs(),
        uptime_secs: started_at.elapsed().as_secs(),
        started_at_ms: Some(started_at_ms),
        config: config.clone(),
    };
    serde_json::to_value(&status).unwrap_or(Value::Null)
}

/// peer_dial：登记地址 + 连接（语义同 GUI AppState::dial）；失败仍返回报告（ok=false）。
pub async fn dial(node: &Node, target: &str) -> Result<Value, String> {
    let (peer, addr) = parse_target(target)?;
    node.add_peer_address(peer, &addr)
        .map_err(|e| format!("登记对端地址失败: {e}"))?;
    connect_report(node, peer).await
}

/// peer_connect：按地址簿直连（语义同 GUI AppState::connect）。
pub async fn connect(node: &Node, peer_id: &str) -> Result<Value, String> {
    let peer = parse_peer_id(peer_id)?;
    connect_report(node, peer).await
}

async fn connect_report(node: &Node, peer: PeerId) -> Result<Value, String> {
    let mut rx = node.events();
    let started = Instant::now();
    let result = node.connect(peer).await;
    let hops = drain_hops(&mut rx);
    let report = match result {
        Ok(()) => DialReport {
            peer: peer.to_string(),
            hops,
            ok: true,
            total_ms: elapsed_ms(started),
        },
        Err(e) => {
            eprintln!("p2pctl-daemon: 连接 {peer} 失败: {e}");
            DialReport {
                peer: peer.to_string(),
                hops,
                ok: false,
                total_ms: elapsed_ms(started),
            }
        }
    };
    Ok(serde_json::to_value(&report).unwrap_or(Value::Null))
}

/// peer_disconnect：幂等，返回是否确有连接被关闭。
pub fn disconnect(node: &Node, peer_id: &str) -> Result<Value, String> {
    let peer = parse_peer_id(peer_id)?;
    let disconnected = node.disconnect(&peer);
    Ok(json!({ "peer": peer.to_string(), "disconnected": disconnected }))
}

/// peer_ping：echo 协议测 RTT（语义同 GUI AppState::ping，含逐跳回收）。
pub async fn ping(node: &Node, peer_id: &str, timeout_ms: u64) -> Result<Value, String> {
    if timeout_ms == 0 {
        return Err("timeoutMs 必须为正数".into());
    }
    let peer = parse_peer_id(peer_id)?;
    let mut rx = node.events();
    let id = ProtocolId::new(p2p_cli::echo::ECHO_PROTOCOL)
        .map_err(|e| format!("内置 echo 协议 id 非法: {e}"))?;
    let started = Instant::now();
    let mut hops = Vec::new();
    let request = node.request(peer, id, p2p_cli::echo::PING_PAYLOAD.to_vec(), Duration::from_millis(timeout_ms));
    tokio::pin!(request);
    let reply = loop {
        tokio::select! {
            reply = &mut request => break reply.map_err(|e| e.to_string()),
            event = rx.recv() => match event {
                Ok(NodeEvent::DialHop { hop, ok, detail, .. }) =>
                    hops.push(DialHopJson { hop: HopKind::from(hop), ok, detail }),
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    eprintln!("p2pctl-daemon: ping 期间事件通道关闭");
                    break Err("事件通道关闭".into());
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            },
        }
    };
    let outcome = match reply {
        Ok(data) if data == p2p_cli::echo::PING_PAYLOAD => PingOutcome {
            ok: true,
            rtt_ms: Some(elapsed_ms(started)),
            hops,
            error: None,
        },
        Ok(data) => PingOutcome {
            ok: false,
            rtt_ms: Some(elapsed_ms(started)),
            hops,
            error: Some(format!("echo 应答与请求不符（实得 {} 字节）", data.len())),
        },
        Err(e) => PingOutcome {
            ok: false,
            rtt_ms: None,
            hops,
            error: Some(format!("echo 请求失败: {e}")),
        },
    };
    Ok(serde_json::to_value(&outcome).unwrap_or(Value::Null))
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// metrics_get：运行时指标快照（语义同 GUI AppState::metrics 运行分支）。
pub fn metrics(node: &Node) -> Result<Value, String> {
    let snapshot = MetricsJson::from(node.metrics());
    serde_json::to_value(&snapshot).map_err(|e| format!("指标快照序列化失败: {e}"))
}

/// 收缓冲区内 DialHop 事件（同 GUI drain_hops）。
fn drain_hops(rx: &mut tokio::sync::broadcast::Receiver<NodeEvent>) -> Vec<DialHopJson> {
    let mut hops = Vec::new();
    loop {
        match rx.try_recv() {
            Ok(NodeEvent::DialHop { hop, ok, detail, .. }) => {
                hops.push(DialHopJson { hop: HopKind::from(hop), ok, detail });
            }
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
            Err(tokio::sync::broadcast::error::TryRecvError::Empty
            | tokio::sync::broadcast::error::TryRecvError::Closed) => break,
        }
    }
    hops
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_peer() -> String {
        bs58::encode([7u8; 32]).into_string()
    }

    #[test]
    fn parse_target_accepts_quic_and_tcp() {
        let (peer, addr) = parse_target(&format!("{}@127.0.0.1/u3400", sample_peer())).unwrap();
        assert_eq!(peer.to_string(), sample_peer());
        assert_eq!(addr, "127.0.0.1/u3400");
        assert!(parse_target(&format!("{}@::1/t3401", sample_peer())).is_ok());
    }

    #[test]
    fn parse_target_rejects_malformed() {
        assert!(parse_target("no-separator").is_err());
        assert!(parse_target(&format!("{}@1.2.3.4/3400", sample_peer())).is_err());
        assert!(parse_target(&format!("{}@1.2.3.4/u0", sample_peer())).is_err());
    }
}
