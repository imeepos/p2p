//! M3 验收（coordination.md S 包）：design §7.3 降级链贯通——
//! 直连被人为阻断 → 打洞（信令闭环 + 探测失败）→ 回落中继电路 → 上层 echo 成功；
//! 每一跳的结果在事件流中断言（Direct/Punch/Relay）。
//!
//! 夹具红线（R 审查）：relay 服务端链路的 peer_id 必须是对端真实身份
//!（取握手互认的 conn.remote），标成 relay 自身会让属主/配额校验失效。

use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use p2p::{Node, NodeEvent, ProtocolHandler, ProtocolId};
use p2p_mux::BoxedStream;
use p2p_protocol::{read_frame, write_frame};
use p2p_relay::{LinkSource, RelayLimits, RelayLink, RelayService, RelayServiceImpl};
use p2p_transport::{QuicTransport, TcpTransport};
use tokio::sync::{broadcast, mpsc};

const ECHO_PROTOCOL: &str = "/test/echo/1";

struct Echo;

#[async_trait::async_trait]
impl ProtocolHandler for Echo {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(ECHO_PROTOCOL).expect("valid protocol id")
    }

    async fn handle(&self, mut stream: BoxedStream) -> io::Result<()> {
        let req = read_frame(&mut stream).await?;
        write_frame(&mut stream, &req).await
    }
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p2p-m3-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    dir
}

/// 进程内 relay 服务端（bootstrap 角色演练）：独立监听，连接即链路，
/// peer_id = 握手互认的对端身份（红线）。
async fn spawn_relay_server() -> io::Result<String> {
    let (tx, rx) = mpsc::channel::<Box<dyn RelayLink>>(32);
    let svc = Arc::new(RelayServiceImpl::new(
        Box::new(LinkChanSource::new(rx)),
        RelayLimits::default(),
    ));
    tokio::spawn(async move {
        if let Err(e) = RelayService::serve(svc).await {
            tracing::error!(error = %e, "relay service exited");
        }
    });

    let keypair = p2p_identity::Keypair::generate();
    let quic = QuicTransport::bind(sock(0), &keypair).await?;
    let quic_addr = quic.local_addr()?;
    tokio::spawn(accept_quic(quic, tx.clone()));
    let tcp = TcpTransport::new();
    let listener = tcp.bind(sock(0)).await?;
    let _tcp_addr = listener.local_addr()?;
    tokio::spawn(accept_tcp(tcp, listener, keypair, tx));
    Ok(format!("127.0.0.1/u{}", quic_addr.port()))
}

fn sock(port: u16) -> SocketAddr {
    SocketAddr::new(std::net::IpAddr::from([127, 0, 0, 1]), port)
}

struct LinkChanSource {
    rx: tokio::sync::Mutex<mpsc::Receiver<Box<dyn RelayLink>>>,
}

impl LinkChanSource {
    fn new(rx: mpsc::Receiver<Box<dyn RelayLink>>) -> Self {
        Self {
            rx: tokio::sync::Mutex::new(rx),
        }
    }
}

#[async_trait::async_trait]
impl LinkSource for LinkChanSource {
    async fn next_link(&self) -> Option<Box<dyn RelayLink>> {
        self.rx.lock().await.recv().await
    }
}

async fn accept_quic(quic: QuicTransport, tx: mpsc::Sender<Box<dyn RelayLink>>) {
    while let Some(conn) = quic.accept().await {
        let link: Box<dyn RelayLink> = Box::new(RemoteLink {
            peer: conn.remote.to_string(),
            mux: conn.mux,
        });
        if tx.send(link).await.is_err() {
            return;
        }
    }
}

async fn accept_tcp(
    tcp: TcpTransport,
    listener: tokio::net::TcpListener,
    keypair: p2p_identity::Keypair,
    tx: mpsc::Sender<Box<dyn RelayLink>>,
) {
    loop {
        match tcp.accept(&listener, &keypair).await {
            Ok(conn) => {
                let link: Box<dyn RelayLink> = Box::new(RemoteLink {
                    peer: conn.remote.to_string(),
                    mux: conn.mux,
                });
                if tx.send(link).await.is_err() {
                    return;
                }
            }
            Err(e) => tracing::warn!(error = %e, "relay tcp accept failed"),
        }
    }
}

/// 服务端侧链路：peer_id 必须是对端真实身份（红线）。
struct RemoteLink {
    peer: String,
    mux: Arc<dyn p2p_mux::MuxControl>,
}

#[async_trait::async_trait]
impl RelayLink for RemoteLink {
    fn peer_id(&self) -> &str {
        &self.peer
    }

    async fn open_stream(&self) -> io::Result<BoxedStream> {
        self.mux.open_stream().await
    }

    async fn accept_stream(&self) -> Option<BoxedStream> {
        self.mux.accept_stream().await
    }
}

async fn expect_event(
    rx: &mut broadcast::Receiver<NodeEvent>,
    want: &dyn Fn(&NodeEvent) -> bool,
    what: &str,
) -> NodeEvent {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let budget = deadline.saturating_duration_since(tokio::time::Instant::now());
        let ev = tokio::time::timeout(budget, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
            .expect("event channel open");
        if want(&ev) {
            return ev;
        }
    }
}

/// 全链验收：直连阻断 → 打洞信令闭环但探测失败 → 中继电路兜底 → echo 成功。
#[tokio::test]
async fn degradation_chain_lands_on_relay_circuit() {
    let relay_addr = spawn_relay_server().await.expect("relay server");

    let a_dir = tmp_dir("a");
    let b_dir = tmp_dir("b");
    // 双方宣告不可达地址：直连与打洞探测均被人为阻断，逼出中继兜底
    let node_a = Node::builder()
        .mdns(false)
        .data_dir(a_dir.clone())
        .relay_addrs(vec![relay_addr.clone()])
        .advertised_addrs(vec!["127.0.0.1/t1".to_string()])
        .build()
        .await
        .expect("node a");
    let node_b = Node::builder()
        .mdns(false)
        .data_dir(b_dir.clone())
        .relay_addrs(vec![relay_addr])
        .advertised_addrs(vec!["127.0.0.1/t2".to_string()])
        .build()
        .await
        .expect("node b");
    let a_peer = node_a.local_peer_id();

    node_a.handle_protocol(Arc::new(Echo));

    let mut events_b = node_b.events();
    // 直连阻断：B 地址簿里 A 的地址指向未监听端口
    node_b
        .add_peer_address(a_peer, "127.0.0.1/t9")
        .expect("seed blocked addr");

    let reply = node_b
        .request(
            a_peer,
            ProtocolId::new(ECHO_PROTOCOL).expect("id"),
            b"m3-chain".to_vec(),
            Duration::from_secs(30),
        )
        .await
        .expect("echo over relay circuit");
    assert_eq!(reply, b"m3-chain");

    // 逐跳事件断言：直连失败 → 打洞失败 → 中继成功（design §12 禁止静默降级）
    expect_event(
        &mut events_b,
        &|ev| {
            matches!(
                ev,
                NodeEvent::DialHop {
                    hop: p2p_swarm::DialHop::Direct,
                    ok: false,
                    ..
                }
            )
        },
        "Direct hop failure event",
    )
    .await;
    expect_event(
        &mut events_b,
        &|ev| {
            matches!(
                ev,
                NodeEvent::DialHop {
                    hop: p2p_swarm::DialHop::Punch,
                    ok: false,
                    ..
                }
            )
        },
        "Punch hop failure event",
    )
    .await;
    expect_event(
        &mut events_b,
        &|ev| {
            matches!(
                ev,
                NodeEvent::DialHop {
                    hop: p2p_swarm::DialHop::Relay,
                    ok: true,
                    ..
                }
            )
        },
        "Relay hop success event",
    )
    .await;
    expect_event(
        &mut events_b,
        &|ev| matches!(ev, NodeEvent::PeerConnected { peer } if *peer == a_peer),
        "PeerConnected over circuit",
    )
    .await;

    node_a.shutdown();
    node_b.shutdown();
    let _ = std::fs::remove_dir_all(&a_dir);
    let _ = std::fs::remove_dir_all(&b_dir);
}
