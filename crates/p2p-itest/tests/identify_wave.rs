//! identify/1 互通波（INTEROP IV2）：双节点一问一答语义四用例。
//!
//! 1. 基础交换：Response.pubkey == B 身份、listen_addrs 含 B 监听地址、
//!    observed_addr == B 对 A 的连接远端观测、software 版本串；
//! 2. 身份交叉核对失败断流：响应 pubkey 与握手 PeerId 不一致即报错断流；
//! 3. 空 listen_addrs 合法性：纯客户端形态响应（无监听/无观测/无 software）可接受；
//! 4. protocol_version 不匹配拒绝：服务端关流无响应，客户端以 EOF 类错误收敛
//!    （真实 handler 的版本拒绝逻辑另由 p2p-swarm 单元测试覆盖）。

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use p2p_identity::{Keypair, PeerId};
use p2p_mux::BoxedStream;
use p2p_protocol::identify::{Request, Response, PROTOCOL_VERSION};
use p2p_protocol::{read_frame, write_frame, HandlerRegistry, ProtocolHandler, ProtocolId};
use p2p_swarm::{Swarm, SwarmConfig, IDENTIFY_PROTOCOL};
use p2p_transport::TransportAddr;
use prost::Message;

const SETTLE: Duration = Duration::from_secs(1);

fn ip_of(addr: &TransportAddr) -> IpAddr {
    match addr {
        TransportAddr::Quic { ip, .. } | TransportAddr::Tcp { ip, .. } => *ip,
    }
}

fn port_of(addr: &TransportAddr) -> u16 {
    match addr {
        TransportAddr::Quic { port, .. } | TransportAddr::Tcp { port, .. } => *port,
    }
}

fn config() -> SwarmConfig {
    SwarmConfig {
        keypair: Arc::new(Keypair::generate()),
        quic_port: 0,
        tcp_port: 0,
        registry: Arc::new(HandlerRegistry::default()),
        relay_addrs: Vec::new(),
        advertised_addrs: Vec::new(),
    }
}

/// 带自定义 registry 的节点配置：抢注 identify 即压制内置注入（测试钩子）。
fn config_with(handler: Arc<dyn ProtocolHandler>) -> SwarmConfig {
    let mut registry = HandlerRegistry::default();
    registry.register(handler);
    SwarmConfig {
        registry: Arc::new(registry),
        ..config()
    }
}

async fn pair_ab(b_cfg: SwarmConfig) -> (Arc<Swarm>, Arc<Swarm>, PeerId) {
    let a = Swarm::start(config()).await.expect("bind a");
    let b = Swarm::start(b_cfg).await.expect("bind b");
    let peer_b = b.local_peer_id();
    a.add_peer_addresses(peer_b, b.listen_addrs());
    (a, b, peer_b)
}

/// 应答伪 pubkey 的恶意节点：身份嫁接必须被客户端交叉核对拦截。
struct WrongPubkey;

#[async_trait::async_trait]
impl ProtocolHandler for WrongPubkey {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(IDENTIFY_PROTOCOL).expect("valid id")
    }
    async fn handle(&self, mut stream: BoxedStream) -> std::io::Result<()> {
        let req = read_frame(&mut stream).await?;
        let _ = Request::decode(req.as_slice()).expect("decode request");
        let resp = Response {
            pubkey: vec![7u8; 32],
            ..Response::default()
        };
        write_frame(&mut stream, &resp.encode_to_vec()).await
    }
}

/// 纯客户端形态应答：空监听、无观测、无 software——全部合法。
struct MinimalResponder {
    pubkey: [u8; 32],
}

#[async_trait::async_trait]
impl ProtocolHandler for MinimalResponder {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(IDENTIFY_PROTOCOL).expect("valid id")
    }
    async fn handle(&self, mut stream: BoxedStream) -> std::io::Result<()> {
        let req = read_frame(&mut stream).await?;
        let decoded = Request::decode(req.as_slice()).expect("decode request");
        assert_eq!(decoded.protocol_version, PROTOCOL_VERSION);
        let resp = Response {
            pubkey: self.pubkey.to_vec(),
            ..Response::default()
        };
        write_frame(&mut stream, &resp.encode_to_vec()).await
    }
}

/// 模拟版本不兼容服务端：读请求后不开口直接关流（显式拒绝语义）。
struct DeafIdentify;

#[async_trait::async_trait]
impl ProtocolHandler for DeafIdentify {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(IDENTIFY_PROTOCOL).expect("valid id")
    }
    async fn handle(&self, mut stream: BoxedStream) -> std::io::Result<()> {
        let _ = read_frame(&mut stream).await?;
        Ok(())
    }
}

#[tokio::test]
async fn basic_exchange_pubkey_listen_addrs_and_observed_addr() {
    let (a, b, peer_b) = pair_ab(config()).await;
    a.connect(peer_b).await.expect("dial b");

    let info = a.identify(peer_b).await.expect("identify roundtrip");
    assert_eq!(info.peer, peer_b);
    // 身份：响应 pubkey 推导必须等于 B 的握手身份
    assert_eq!(
        PeerId::from_public_key(&info.pubkey),
        peer_b,
        "pubkey must cross-check against handshake identity"
    );
    // 监听地址：B 的 QUIC+TCP 监听全量在列
    let listen = b.listen_addrs();
    assert_eq!(info.listen_addrs.len(), listen.len());
    for addr in &listen {
        assert!(
            info.listen_addrs.contains(addr),
            "listen_addrs must contain {addr}"
        );
    }
    // 观测地址：B 看到的 A 连接远端。A 侧拿不到自身拨号源端口，断言
    // 语义要素（在、回环、端口有效、随池化连接稳定）；精确填值由
    // p2p-swarm identify 单元测试覆盖。
    let observed = info
        .observed_addr
        .clone()
        .expect("direct conn must observe remote");
    assert_eq!(ip_of(&observed).to_string(), "127.0.0.1");
    assert!(port_of(&observed) > 0);
    assert!(
        !listen.iter().any(|l| port_of(l) == port_of(&observed)),
        "observed port must be A's dial source, not B's listen port"
    );
    assert_eq!(
        info.software.as_deref(),
        Some("p2p-base/0.1.0"),
        "software must report p2p-base/version"
    );
    // 同一池化连接重复询问：观测端口稳定（同一 socket 对）
    let again = a.identify(peer_b).await.expect("second roundtrip");
    assert_eq!(again.observed_addr, info.observed_addr);
}

#[tokio::test]
async fn pubkey_crosscheck_mismatch_breaks_stream() {
    let (a, _b, peer_b) = pair_ab(config_with(Arc::new(WrongPubkey))).await;
    a.connect(peer_b).await.expect("dial b");
    let err = a
        .identify(peer_b)
        .await
        .expect_err("rogue pubkey must be rejected");
    assert!(
        err.to_string().contains("mismatch"),
        "error must attribute pubkey mismatch, got: {err}"
    );
}

#[tokio::test]
async fn empty_listen_addrs_is_legal_client_shape() {
    let b_key = Keypair::generate();
    let responder = MinimalResponder {
        pubkey: b_key.public(),
    };
    let mut cfg = config_with(Arc::new(responder));
    // 关键：B 的身份要与响应 pubkey 一致（交叉核对通过的前提）
    cfg.keypair = Arc::new(b_key);
    let (a, _b, peer_b) = pair_ab(cfg).await;
    let info = a.identify(peer_b).await.expect("minimal response is legal");
    assert_eq!(info.listen_addrs, Vec::new(), "empty listen_addrs is legal");
    assert!(info.observed_addr.is_none(), "observed absent is legal");
    assert_eq!(info.software, None, "software absent is legal");
    assert_eq!(PeerId::from_public_key(&info.pubkey), peer_b);
}

#[tokio::test]
async fn version_incompatible_server_rejects_without_response() {
    let (a, _b, peer_b) = pair_ab(config_with(Arc::new(DeafIdentify))).await;
    a.connect(peer_b).await.expect("dial b");
    let err = a.identify(peer_b).await.expect_err("rejection must error");
    assert!(
        matches!(err, p2p_protocol::ProtocolError::Io(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof),
        "server closing without response must surface as EOF-type error, got: {err}"
    );
    // SETTLE 只为保证日志冲刷观测稳定，不影响断言
    tokio::time::sleep(SETTLE).await;
}
