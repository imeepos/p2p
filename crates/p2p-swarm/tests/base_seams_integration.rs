//! BASE1 底座接缝回归：同向重拨收敛（多查号）与入站流身份下传。
//!
//! T23 两机冒烟根因回归：对端同身份重拨时，服务端池内旧条目（半开残留）
//! 不得把新连接判落选 close；入站分发随流下传握手互认的 PeerId。

use std::io;
use std::sync::Arc;
use std::time::Duration;

use p2p_identity::{Keypair, PeerId};
use p2p_mux::BoxedStream;
use p2p_protocol::{
    open_with_protocol, read_frame, write_frame, HandlerRegistry, ProtocolHandler, ProtocolId,
};
use p2p_swarm::{Swarm, SwarmConfig};
use tokio::io::AsyncWriteExt;

const PROTO: &str = "/test/base1/echo/1";

fn protocol_id() -> ProtocolId {
    ProtocolId::new(PROTO).expect("static protocol id is valid")
}

fn config(registry: HandlerRegistry) -> SwarmConfig {
    SwarmConfig {
        keypair: Arc::new(Keypair::generate()),
        quic_port: 0,
        tcp_port: 0,
        registry: Arc::new(registry),
        relay_addrs: Vec::new(),
        advertised_addrs: Vec::new(),
    }
}

/// 回显 + 身份捕获：handle_inbound 记录分发层下传的对端身份后回显一帧；
/// 裸流入口到达即失败，证明 swarm 分发只走带身份接缝。
struct EchoCapture {
    peers: tokio::sync::mpsc::UnboundedSender<PeerId>,
}

#[async_trait::async_trait]
impl ProtocolHandler for EchoCapture {
    fn protocol(&self) -> ProtocolId {
        protocol_id()
    }

    async fn handle_inbound(&self, peer: PeerId, mut stream: BoxedStream) -> io::Result<()> {
        let _ = self.peers.send(peer);
        let req = read_frame(&mut stream).await?;
        write_frame(&mut stream, &req).await?;
        stream.flush().await
    }

    async fn handle(&self, _stream: BoxedStream) -> io::Result<()> {
        Err(io::Error::other("swarm dispatch must use handle_inbound"))
    }
}

async fn spawn_server() -> (
    Arc<Swarm>,
    PeerId,
    tokio::sync::mpsc::UnboundedReceiver<PeerId>,
) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let mut registry = HandlerRegistry::default();
    registry.register(Arc::new(EchoCapture { peers: tx }));
    let server = Swarm::start(config(registry)).await.expect("server binds");
    let server_peer = server.local_peer_id();
    (server, server_peer, rx)
}

fn tcp_addr(swarm: &Swarm) -> p2p_transport::TransportAddr {
    swarm
        .listen_addrs()
        .into_iter()
        .find(|a| matches!(a, p2p_transport::TransportAddr::Tcp { .. }))
        .expect("server tcp listen addr")
}

async fn roundtrip(client: &Swarm, server_peer: PeerId, payload: &[u8]) {
    let raw = client
        .open_stream(&server_peer, &protocol_id())
        .await
        .expect("open stream");
    let mut stream = open_with_protocol(raw, &protocol_id())
        .await
        .expect("protocol handshake");
    write_frame(&mut stream, payload)
        .await
        .expect("write frame");
    let resp = tokio::time::timeout(Duration::from_secs(10), read_frame(&mut stream))
        .await
        .expect("echo within timeout")
        .expect("echo read");
    assert_eq!(resp, payload.to_vec(), "echo roundtrip must match");
}

/// 多查号回归：同身份先注册连接、重拨后的新连接必须照常被服务——
/// 服务端池内旧条目（半开残留窗口）不得把新连接判落选 close
/// （T23 两机冒烟第二借方 10s 握手超时同根因）。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn multi_query_redial_roundtrip_after_reconnect() {
    let (server, server_peer, _rx) = spawn_server().await;
    let client = Swarm::start(config(HandlerRegistry::default()))
        .await
        .expect("client binds");
    client.add_peer_addresses(server_peer, vec![tcp_addr(&server)]);

    client.connect(server_peer).await.expect("first connect");
    roundtrip(&client, server_peer, b"query-1").await;

    // 本端挂断后立即重拨：服务端可能尚未处理完旧连接关闭（竞态窗口）
    assert!(client.disconnect(&server_peer), "first conn must be pooled");
    client.connect(server_peer).await.expect("redial");
    roundtrip(&client, server_peer, b"query-2").await;
    server.shutdown();
}

/// 身份下传回归：handler 拿到的 PeerId 必须等于拨号方握手互认身份。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn peer_passthrough_serves_real_remote_identity() {
    let (server, server_peer, mut rx) = spawn_server().await;
    let client = Swarm::start(config(HandlerRegistry::default()))
        .await
        .expect("client binds");
    client.add_peer_addresses(server_peer, vec![tcp_addr(&server)]);

    client.connect(server_peer).await.expect("connect");
    roundtrip(&client, server_peer, b"who").await;

    let seen = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("handler invoked within timeout")
        .expect("peer recorded");
    assert_eq!(
        seen,
        client.local_peer_id(),
        "handler must receive the dialer handshake identity"
    );
    server.shutdown();
}
