//! bootstrap 子命令：rendezvous（facade 节点自带）+ relay 服务端装配。
//!
//! rendezvous 走 facade Node 的协议分发（/p2p-base/rendezvous/1）；relay 走独立
//! 传输监听（每连接=一条 RelayLink，流为裸 RelayMsg，与 rendezvous 的协议 ID 分帧
//! 互不兼容，故分开监听端口，详见最终报告设计决策）。

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use p2p::NodeBuilder;
use p2p_identity::Keypair;
use p2p_mux::{BoxedStream, MuxControl};
use p2p_relay::{LinkSource, RelayLimits, RelayLink, RelayService, RelayServiceImpl};
use p2p_transport::{QuicTransport, TcpTransport};
use tokio::sync::{broadcast, mpsc};

use crate::cli::{parse_socket_addr, BootstrapArgs};

/// 单条已认证传输连接 = 一条 RelayLink（peer 为握手互认的身份）。
struct RelayConnLink {
    peer: String,
    mux: Arc<dyn MuxControl>,
}

#[async_trait::async_trait]
impl RelayLink for RelayConnLink {
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

/// 传输链路源：两条 accept 循环把新连接推入 channel，serve 逐条取用。
struct RelayConnSource {
    rx: tokio::sync::Mutex<mpsc::Receiver<Box<dyn RelayLink>>>,
}

impl RelayConnSource {
    fn new(rx: mpsc::Receiver<Box<dyn RelayLink>>) -> Self {
        Self {
            rx: tokio::sync::Mutex::new(rx),
        }
    }
}

#[async_trait::async_trait]
impl LinkSource for RelayConnSource {
    async fn next_link(&self) -> Option<Box<dyn RelayLink>> {
        self.rx.lock().await.recv().await
    }
}

/// 装配并常驻：facade 节点（rendezvous）+ relay 服务，ctrl-c 优雅退出。
pub async fn run(args: BootstrapArgs) -> Result<(), String> {
    let quic_addr = parse_socket_addr(&args.listen_quic)?;
    let tcp_addr = parse_socket_addr(&args.listen_tcp)?;

    let node = NodeBuilder::new()
        .mdns(false)
        .data_dir(PathBuf::from(&args.data))
        .quic_port(quic_addr.port())
        .tcp_port(tcp_addr.port())
        .observation_responder(args.observation_port)
        .build()
        .await
        .map_err(|e| format!("rendezvous/监听装配失败: {e}"))?;

    let peer = node.local_peer_id();
    let keypair =
        load_identity(PathBuf::from(&args.data)).map_err(|e| format!("读取身份失败: {e}"))?;
    let rendezvous_addrs = node.listen_addrs();
    // relay 用 +3 偏移：+2 的 UDP 口已被观测反射器占用（observation_port）
    let relay_quic = SocketAddr::new(IpAddr::from([0, 0, 0, 0]), quic_addr.port() + 3);
    let relay_tcp = SocketAddr::new(IpAddr::from([0, 0, 0, 0]), tcp_addr.port() + 3);
    spawn_relay(keypair.clone(), relay_quic, relay_tcp)
        .await
        .map_err(|e| format!("relay 监听/装配失败: {e}"))?;

    println!("peer_id={peer}");
    println!(
        "rendezvous (QUIC {} / TCP {})",
        quic_addr.port(),
        tcp_addr.port()
    );
    println!("rendezvous_addrs={rendezvous_addrs:?}");
    println!(
        "relay (QUIC {} / TCP {})",
        relay_quic.port(),
        relay_tcp.port()
    );

    let mut events = node.events();
    let mut ctrl_c = Box::pin(tokio::signal::ctrl_c());
    loop {
        tokio::select! {
            _ = &mut ctrl_c => {
                tracing::info!("ctrl-c received, shutting down");
                node.shutdown();
                return Ok(());
            }
            ev = events.recv() => match ev {
                Ok(ev) => tracing::debug!(event = ?ev, "bootstrap event"),
                Err(broadcast::error::RecvError::Lagged(skip)) =>
                    tracing::warn!(skip, "event channel lagged"),
                Err(broadcast::error::RecvError::Closed) => {
                    node.shutdown();
                    return Ok(());
                }
            }
        }
    }
}

/// 从 data 目录加载或生成身份（与 facade 同一 key.seed，确保 PeerId 一致）。
fn load_identity(data: PathBuf) -> io::Result<Keypair> {
    std::fs::create_dir_all(&data)?;
    p2p_identity::load_or_generate_seed(&data.join("key.seed"))
}

/// 启动 relay：绑 quic/tcp 监听、两条 accept 循环喂 LinkSource，后台 serve。
async fn spawn_relay(
    keypair: Keypair,
    quic_addr: SocketAddr,
    tcp_addr: SocketAddr,
) -> io::Result<()> {
    let (tx, rx) = mpsc::channel::<Box<dyn RelayLink>>(32);
    let source = Box::new(RelayConnSource::new(rx));
    let svc = Arc::new(RelayServiceImpl::new(source, RelayLimits::default()));
    tokio::spawn(async move {
        if let Err(e) = RelayService::serve(svc).await {
            tracing::error!(error = %e, "relay service exited with error");
        }
    });

    let quic = QuicTransport::bind(quic_addr, &keypair).await?;
    tokio::spawn(accept_quic_loop(quic, tx.clone()));

    let tcp = TcpTransport::new();
    let listener = tcp.bind(tcp_addr).await?;
    tokio::spawn(accept_tcp_loop(tcp, listener, keypair, tx));

    Ok(())
}

/// QUIC accept 循环：升级为 SecureConn 后包装成 RelayLink 推入 source。
async fn accept_quic_loop(quic: QuicTransport, tx: mpsc::Sender<Box<dyn RelayLink>>) {
    loop {
        match quic.accept().await {
            Some(conn) => {
                let link: Box<dyn RelayLink> = Box::new(RelayConnLink {
                    peer: conn.remote.to_string(),
                    mux: conn.mux,
                });
                if tx.send(link).await.is_err() {
                    tracing::info!("relay link channel closed; quic accept loop exits");
                    return;
                }
            }
            None => {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }
    }
}

/// TCP accept 循环：升级为 SecureConn 后包装成 RelayLink 推入 source。
async fn accept_tcp_loop(
    tcp: TcpTransport,
    listener: tokio::net::TcpListener,
    keypair: Keypair,
    tx: mpsc::Sender<Box<dyn RelayLink>>,
) {
    loop {
        match tcp.accept(&listener, &keypair).await {
            Ok(conn) => {
                let link: Box<dyn RelayLink> = Box::new(RelayConnLink {
                    peer: conn.remote.to_string(),
                    mux: conn.mux,
                });
                if tx.send(link).await.is_err() {
                    tracing::info!("relay link channel closed; tcp accept loop exits");
                    return;
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "tcp accept failed");
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }
    }
}
