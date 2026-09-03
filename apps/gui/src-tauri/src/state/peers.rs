//! peer 操作（gui-contract.md §1/§6）：peer_dial/peer_connect 逐跳回收、
//! peer_ping 测距与 peer_disconnect 挂断。

use std::time::{Duration, Instant};

use p2p::{Node, NodeError, NodeEvent, PeerId, ProtocolId};
use tokio::sync::broadcast;
use tracing::warn;

use super::{elapsed_ms, AppState};
use crate::proto;
use crate::types::{DialHopJson, DialReport, PingOutcome};

impl AppState {
    /// peer_dial：登记地址 + 连接，回收期间 DialHop 为逐跳报告（契约 §1/§6）。
    pub(crate) async fn dial(&self, target: &str) -> Result<DialReport, String> {
        let (peer, addr) = proto::parse_target(target)?;
        let node = self.running_node().await?;
        node.add_peer_address(peer, &addr).map_err(|e| {
            warn!(error = %e, peer = %peer, "登记对端地址失败");
            format!("登记对端地址失败: {e}")
        })?;
        let mut rx = node.events();
        let started = Instant::now();
        let result = node.connect(peer).await;
        let hops = drain_hops(&mut rx);
        match result {
            Ok(()) => Ok(DialReport {
                peer: peer.to_string(),
                hops,
                ok: true,
                total_ms: elapsed_ms(started),
            }),
            Err(e) => {
                warn!(error = %e, peer = %peer, "连接对端失败");
                Ok(DialReport {
                    peer: peer.to_string(),
                    hops,
                    ok: false,
                    total_ms: elapsed_ms(started),
                })
            }
        }
    }

    /// peer_connect：按地址簿直连已知节点（免重复登记），逐跳报告同 peer_dial。
    pub(crate) async fn connect(&self, peer_id: &str) -> Result<DialReport, String> {
        let peer = proto::parse_peer_id(peer_id)?;
        let node = self.running_node().await?;
        let mut rx = node.events();
        let started = Instant::now();
        let result = node.connect(peer).await;
        let hops = drain_hops(&mut rx);
        match result {
            Ok(()) => Ok(DialReport {
                peer: peer.to_string(),
                hops,
                ok: true,
                total_ms: elapsed_ms(started),
            }),
            Err(e) => {
                warn!(error = %e, peer = %peer, "连接已知节点失败");
                Ok(DialReport {
                    peer: peer.to_string(),
                    hops,
                    ok: false,
                    total_ms: elapsed_ms(started),
                })
            }
        }
    }

    /// peer_disconnect：挂断与该节点的连接；幂等，未在册连接返回 false。
    pub(crate) async fn disconnect(&self, peer_id: &str) -> Result<bool, String> {
        let peer = proto::parse_peer_id(peer_id)?;
        let node = self.running_node().await?;
        Ok(node.disconnect(&peer))
    }

    /// peer_ping：复用 echo 协议 request（同 p2p-cli ping），期间逐跳一并回收。
    pub(crate) async fn ping(&self, peer_id: &str, timeout_ms: u64) -> Result<PingOutcome, String> {
        if timeout_ms == 0 {
            return Err("timeoutMs 必须为正数".into());
        }
        let peer = proto::parse_peer_id(peer_id)?;
        let node = self.running_node().await?;
        let mut rx = node.events();
        let id = echo_protocol_id();
        let started = Instant::now();
        let mut hops = Vec::new();
        let reply = request_with_hops(
            &node,
            peer,
            id,
            Duration::from_millis(timeout_ms),
            &mut rx,
            &mut hops,
        )
        .await;
        hops.extend(drain_hops(&mut rx));
        Ok(match reply {
            Ok(data) if data == proto::PING_PAYLOAD => PingOutcome {
                ok: true,
                rtt_ms: Some(elapsed_ms(started)),
                hops,
                error: None,
            },
            Ok(data) => {
                warn!(bytes = data.len(), peer = %peer, "echo 应答与请求不符");
                PingOutcome {
                    ok: false,
                    rtt_ms: Some(elapsed_ms(started)),
                    hops,
                    error: Some(format!("echo 应答与请求不符（实得 {} 字节）", data.len())),
                }
            }
            Err(e) => {
                warn!(error = %e, peer = %peer, "echo 请求失败");
                PingOutcome {
                    ok: false,
                    rtt_ms: None,
                    hops,
                    error: Some(format!("echo 请求失败: {e}")),
                }
            }
        })
    }
}

/// echo request 与 DialHop 事件并发收集（结构同 p2p-cli ping 的 select 循环）。
async fn request_with_hops(
    node: &Node,
    peer: PeerId,
    id: ProtocolId,
    timeout: Duration,
    rx: &mut broadcast::Receiver<NodeEvent>,
    hops: &mut Vec<DialHopJson>,
) -> Result<Vec<u8>, NodeError> {
    let request = node.request(peer, id, proto::PING_PAYLOAD.to_vec(), timeout);
    tokio::pin!(request);
    let mut events_open = true;
    loop {
        tokio::select! {
            reply = &mut request => return reply,
            event = rx.recv(), if events_open => match event {
                Ok(NodeEvent::DialHop { hop, ok, detail, .. }) => {
                    hops.push(DialHopJson { hop: hop.into(), ok, detail });
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(dropped = n, "ping 期间事件积压，部分逐跳丢失");
                }
                Err(broadcast::error::RecvError::Closed) => events_open = false,
            }
        }
    }
}

/// 尽收缓冲区内 DialHop 事件；积压与关闭不致命，留日志。
fn drain_hops(rx: &mut broadcast::Receiver<NodeEvent>) -> Vec<DialHopJson> {
    let mut hops = Vec::new();
    loop {
        match rx.try_recv() {
            Ok(NodeEvent::DialHop { hop, ok, detail, .. }) => {
                hops.push(DialHopJson { hop: hop.into(), ok, detail });
            }
            Ok(_) => {}
            Err(broadcast::error::TryRecvError::Lagged(n)) => {
                warn!(dropped = n, "拨号期间事件积压，部分逐跳丢失");
            }
            Err(broadcast::error::TryRecvError::Empty | broadcast::error::TryRecvError::Closed) => {
                break;
            }
        }
    }
    hops
}

fn echo_protocol_id() -> ProtocolId {
    ProtocolId::new(proto::ECHO_PROTOCOL).expect("内置 echo 协议 id 合法")
}
