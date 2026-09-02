//! E4 回归：跨公网 TCP 引导 /t3401（docs/ops/experiment-env.md §8.5）。
//!
//! 生产链路完整重演：TcpTransport 盲拨（Noise XX + yamux）→ 协议 ID 握手
//! → rendezvous 长度分帧 register/query roundtrip。两端之间插入用户态窄管道泵
//! （小分段 + 每段抖动），模拟公网 MTU/分片与 RTT：任意字节边界都可能截断
//! Noise 帧长前缀/帧体，逼出半帧读、部分写与 Pending 中断路径。
//!
//! 冒烟根因（2026-09-02 诊断）：与公网分段无关——TCP 侧 YamuxMux 为「全部句柄
//! 丢弃即关闭连接」（swarm 门禁/重复连接丢弃依赖该语义），而盲拨路径取流后丢弃
//! SecureConn 即自毁会话（"read stream ended"，服务端同步断）；QUIC 侧 quinn 连接
//! 由驱动任务持有故不受影响。修复归属（facade TransportLink 持有 SecureConn）已报
//! 协调裁决。本回归锚定：句柄被正确持有（修复后形态）时，三 crate 分层在公网
//! 分段/抖动条件下端到端健壮。

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use p2p_discovery::rendezvous::messages::{sign_register, unix_now, Request};
use p2p_discovery::rendezvous::server::{serve_link, RendezvousRegistry};
use p2p_discovery::rendezvous::RendezvousConn;
use p2p_identity::Keypair;
use p2p_mux::BoxedStream;
use p2p_protocol::{
    dispatch_inbound, open_with_protocol, HandlerRegistry, ProtocolHandler, ProtocolId,
};
use p2p_transport::{SecureConn, TcpTransport, Transport, TransportAddr};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use p2p_itest::{expect_within, rendezvous_conn};

const NS: &str = "e4-wan";
const LIMIT: Duration = Duration::from_secs(30);
/// 窄管道：单段远小于 MTU，强制任意边界截断（比真实公网更苛刻）。
const SEGMENT: usize = 256;
/// 每段抖动：模拟公网 RTT，制造读写交替的 Pending 窗口。
const JITTER: Duration = Duration::from_millis(2);
const RENDEZVOUS_PROTOCOL: &str = "/p2p-base/rendezvous/1";
/// 大响应规模：200 条目迫使 Noise 明文跨 8KiB 分帧 + yamux 窗口更新过窄管道。
const BULK_PEERS: usize = 200;

/// 窄管道单向泵：src 读 ≤SEGMENT，原样写 dst 后 flush + 抖动；EOF/错误即半关 dst。
async fn pump<R, W>(mut src: R, mut dst: W)
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = vec![0u8; SEGMENT];
    loop {
        match src.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if dst.write_all(&buf[..n]).await.is_err() || dst.flush().await.is_err() {
                    break;
                }
                tokio::time::sleep(JITTER).await;
            }
        }
    }
    let _ = dst.shutdown().await;
}

/// 门面端口 → 后端传输端口：每个入站连接拆两条泵任务（双向各一）。
async fn spawn_wan_pipe(door: TcpListener, backend: SocketAddr) {
    loop {
        let Ok((client_side, _)) = door.accept().await else {
            break;
        };
        let Ok(server_side) = TcpStream::connect(backend).await else {
            break;
        };
        let (c_r, c_w) = client_side.into_split();
        let (s_r, s_w) = server_side.into_split();
        tokio::spawn(pump(c_r, s_w));
        tokio::spawn(pump(s_r, c_w));
    }
}

/// 生产同构的 rendezvous 协议 handler：入站流帧化后交 serve_link。
struct RendezvousHandler(Arc<RendezvousRegistry>);

#[async_trait::async_trait]
impl ProtocolHandler for RendezvousHandler {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(RENDEZVOUS_PROTOCOL).expect("built-in protocol id is valid")
    }
    async fn handle(&self, stream: BoxedStream) -> io::Result<()> {
        let mut conn = rendezvous_conn(stream);
        serve_link(&mut conn, &self.0)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(())
    }
}

/// bootstrap 服务侧：accept → Noise 升级 → yamux → 收流按注册表分发（listen.rs 同构）。
async fn serve_bootstrap(
    tcp: TcpTransport,
    listener: TcpListener,
    kp: Keypair,
    registry: Arc<RendezvousRegistry>,
) {
    let mut handlers = HandlerRegistry::default();
    handlers.register(Arc::new(RendezvousHandler(registry)));
    let handlers = Arc::new(handlers);
    loop {
        let Ok(conn) = tcp.accept(&listener, &kp).await else {
            continue;
        };
        let mux = conn.mux.clone();
        let handlers = handlers.clone();
        tokio::spawn(async move {
            while let Some(stream) = mux.accept_stream().await {
                let _ = dispatch_inbound(stream, &handlers).await;
            }
        });
    }
}

/// 客户端盲拨（TransportLink::connect 同构）：dial → 开流 → 协议握手 → 分帧连接。
/// 返回 SecureConn 由调用方持有：YamuxMux 语义为句柄归零即断链（契约，
/// swarm 门禁依赖），取流后丢弃句柄会自毁会话——生产 TransportLink 曾踩此坑。
async fn dial_rendezvous(
    tcp: &TcpTransport,
    addr: SocketAddr,
    kp: &Keypair,
) -> (RendezvousConn, SecureConn) {
    let taddr = TransportAddr::Tcp {
        ip: addr.ip(),
        port: addr.port(),
    };
    let conn = expect_within("tcp blind dial", tcp.dial(&taddr, kp, None), LIMIT)
        .await
        .expect("tcp blind dial must succeed");
    let id = ProtocolId::new(RENDEZVOUS_PROTOCOL).expect("built-in protocol id is valid");
    let raw = conn.mux.open_stream().await.expect("open stream");
    let stream = open_with_protocol(raw, &id)
        .await
        .expect("protocol handshake");
    (rendezvous_conn(stream), conn)
}

/// 起 bootstrap + 窄管道，返回客户端拨号入口。
async fn spawn_stack(registry: Arc<RendezvousRegistry>) -> SocketAddr {
    let server_kp = Keypair::generate();
    let tcp = TcpTransport::new();
    let backend = tcp.bind("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let backend_addr = backend.local_addr().unwrap();
    tokio::spawn(serve_bootstrap(tcp, backend, server_kp, registry));

    let door = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let door_addr = door.local_addr().unwrap();
    tokio::spawn(spawn_wan_pipe(door, backend_addr));
    door_addr
}

#[tokio::test]
async fn tcp_rendezvous_roundtrip_survives_wan_segmentation() {
    let registry = Arc::new(RendezvousRegistry::new());
    let door_addr = spawn_stack(registry.clone()).await;

    let client_kp = Keypair::generate();
    let tcp = TcpTransport::new();
    let (mut conn, _session) = dial_rendezvous(&tcp, door_addr, &client_kp).await;

    let advertised = vec![TransportAddr::Quic {
        ip: "10.0.0.1".parse().unwrap(),
        port: 7001,
    }];
    let reg = sign_register(&client_kp, NS, &advertised, 60, unix_now());
    let resp = expect_within(
        "register roundtrip",
        conn.roundtrip(Request::register(reg)),
        LIMIT,
    )
    .await
    .expect("register io");
    resp.ensure_ok().expect("register accepted");

    let q = Request::query(NS.into(), client_kp.peer_id().as_bytes().to_vec());
    let resp = expect_within("self query roundtrip", conn.roundtrip(q), LIMIT)
        .await
        .expect("query io");
    resp.ensure_ok().expect("query ok");
    assert_eq!(resp.peers.len(), 1, "self entry must come back");
    assert_eq!(
        resp.peers[0].peer_id,
        client_kp.peer_id().as_bytes().to_vec()
    );
}

#[tokio::test]
async fn tcp_rendezvous_bulk_query_crosses_noise_chunking() {
    let registry = Arc::new(RendezvousRegistry::new());
    // 进程内预置大注册表（绕过每连接注册限速），迫使查询响应 > 8KiB Noise 单帧。
    for _ in 0..BULK_PEERS {
        let kp = Keypair::generate();
        let addrs = vec![
            TransportAddr::Quic {
                ip: "10.0.0.1".parse().unwrap(),
                port: 7001,
            },
            TransportAddr::Tcp {
                ip: "10.0.0.1".parse().unwrap(),
                port: 7002,
            },
        ];
        let reg = sign_register(&kp, NS, &addrs, 3600, unix_now());
        registry.register(&reg, unix_now()).expect("bulk seed");
    }
    let door_addr = spawn_stack(registry).await;

    let client_kp = Keypair::generate();
    let tcp = TcpTransport::new();
    let (mut conn, _session) = dial_rendezvous(&tcp, door_addr, &client_kp).await;

    let q = Request::query(NS.into(), Vec::new());
    let resp = expect_within("bulk query roundtrip", conn.roundtrip(q), LIMIT)
        .await
        .expect("bulk query io");
    resp.ensure_ok().expect("bulk query ok");
    assert_eq!(
        resp.peers.len(),
        BULK_PEERS,
        "all bulk entries must survive wan segmentation"
    );
}
