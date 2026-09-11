//! W-T2 双节点夹具（真 facade Node TCP 互联 + 进程内 mock 本地目标）。
//! 被访侧 handler 直采 swarm 分发的握手 PeerId（handle_inbound，禁信票据自报身份）；
//! 访侧走 Node 版 StreamFactory 拨号。断言在 tunnel_wave.rs。
#![allow(dead_code)]

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use p2p::Node;
use p2p_identity::PeerId;
use p2p_protocol::{ProtocolId, StreamFactory};
use p2p_tunnel::{
    HttpDialer, TunnelAudit, TunnelAuditRecord, TunnelClient, TunnelError, TunnelErrorCode,
    TunnelGate, TunnelIo, TunnelResponder, TunnelServeConfig, TunnelTicket,
};
use tokio::sync::Mutex;

/// 单步等待上限：本地 loopback 全链毫秒级，15s 为宽松护栏。
pub const STEP: Duration = Duration::from_secs(15);
/// 白名单内目标（精确 127.0.0.1:<port> 形态）。
pub const TARGET: &str = "127.0.0.1:8014";
/// 白名单外目标。
pub const OTHER: &str = "127.0.0.1:9";
/// 票据 uid 夹具（16 hex）。
pub const UID: &str = "0011223344556677";

/// mock 本地目标：dial() 出队一条 duplex 交给 responder，测试侧驱动对端。
#[derive(Clone, Default)]
pub struct MockDialer {
    queue: Arc<Mutex<Vec<tokio::io::DuplexStream>>>,
}

impl MockDialer {
    /// 预置一条目标连接，返回测试侧端（responder 将取得对端）。
    pub async fn feed(&self) -> tokio::io::DuplexStream {
        let (test_side, responder_side) = tokio::io::duplex(64 * 1024);
        self.queue.lock().await.push(responder_side);
        test_side
    }
}

#[async_trait]
impl HttpDialer for MockDialer {
    async fn dial(&self, _target: &str) -> Result<TunnelIo, TunnelError> {
        self.queue
            .lock()
            .await
            .pop()
            .map(|s| Box::new(s) as TunnelIo)
            .ok_or_else(|| TunnelError::Io(std::io::Error::other("no mock target queued")))
    }
}

/// 访侧拨号工厂：裸流交调用方握手（open_raw_stream；工厂禁包 new_stream，
/// 否则流上两帧协议 ID，严格 responder 翻车——2026-09-11 装配 MUST）。
#[derive(Clone)]
pub struct NodeFactory {
    pub node: Arc<Node>,
}

#[async_trait]
impl StreamFactory for NodeFactory {
    async fn open_stream(
        &self,
        peer: &PeerId,
        protocol: &ProtocolId,
    ) -> std::io::Result<p2p::BoxedStream> {
        self.node
            .open_raw_stream(*peer, protocol.clone())
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))
    }
}

pub struct Rig {
    pub a: Arc<Node>,
    pub a_peer: PeerId,
    pub b: Arc<Node>,
    pub dialer: MockDialer,
    pub gate: TunnelGate,
    pub audit: Arc<TunnelAudit>,
    pub client: TunnelClient<NodeFactory>,
    pub root: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.a.shutdown();
        self.b.shutdown();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// 双节点装配：A 挂被访侧 handler（enabled 控制按次开关），B 持访侧客户端。
pub async fn rig(tag: &str, cfg: TunnelServeConfig, enabled: bool) -> Rig {
    let root = std::env::temp_dir().join(format!("tunnel-wave-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let a_dir = root.join("a");
    let b_dir = root.join("b");
    std::fs::create_dir_all(&a_dir).unwrap();
    std::fs::create_dir_all(&b_dir).unwrap();
    let a = Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(a_dir)
            .build()
            .await
            .unwrap(),
    );
    let b = Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(b_dir)
            .build()
            .await
            .unwrap(),
    );
    let a_peer = a.local_peer_id();
    let dialer = MockDialer::default();
    let gate = TunnelGate::new(cfg);
    let responder = Arc::new(TunnelResponder::new(
        p2p_tunnel::protocol_id().unwrap(),
        gate.clone(),
        dialer.clone(),
    ));
    let audit = responder.audit();
    a.handle_protocol(responder);
    if enabled {
        gate.set_enabled(true);
    }
    link(&b, a_peer, &a).await;
    let client = TunnelClient::new(NodeFactory { node: b.clone() });
    Rig {
        a,
        a_peer,
        b,
        dialer,
        gate,
        audit,
        client,
        root,
    }
}

/// 白名单 {TARGET} + 开启态夹具。
pub async fn started_rig(tag: &str) -> Rig {
    let cfg = TunnelServeConfig {
        allowlist: HashSet::from([TARGET.to_string()]),
        ..Default::default()
    };
    rig(tag, cfg, true).await
}

/// 登记对端 TCP 地址并建连（chat_e2e 同款真链路互联）。
async fn link(from: &Node, to_peer: PeerId, to: &Node) {
    for addr in to.listen_addrs().into_iter().filter(|a| a.contains("/t")) {
        from.add_peer_address(to_peer, &addr).unwrap();
    }
    from.connect(to_peer).await.unwrap();
}

pub fn ticket(target: &str) -> TunnelTicket {
    TunnelTicket::new(UID, target, "ab".repeat(16)).unwrap()
}

/// 轮询等被访侧审计落账（responder 在隧道收口后写记录）。
pub async fn wait_audit(rig: &Rig, uid: &str) -> TunnelAuditRecord {
    for _ in 0..100 {
        if let Some(rec) = rig
            .audit
            .snapshot()
            .into_iter()
            .find(|r| r.session_id == uid)
        {
            eprintln!("[audit] {rec:?}");
            return rec;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("audit record for {uid} not recorded");
}

/// 从结构化拒绝里取错误码并留证据输出（其余形态直接爆红）。
pub fn reject_code(err: TunnelError) -> TunnelErrorCode {
    let TunnelError::Rejected { code, message } = err else {
        panic!("expected structured rejection, got {err:?}");
    };
    eprintln!("[evidence] error frame code={code} msg={message:?}");
    code
}

/// open 必须以结构化拒绝收口（TunnelIo 无 Debug，expect_err 不可用）。
pub async fn expect_reject(
    open: impl std::future::Future<Output = Result<p2p_tunnel::TunnelIo, TunnelError>>,
) -> TunnelError {
    match open.await {
        Ok(_) => panic!("expected rejection, got open tunnel"),
        Err(e) => e,
    }
}
