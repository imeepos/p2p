//! relay 服务传输装配（bootstrap 与 metrics 子命令共享）：
//! 两条 accept 循环把已认证连接包装成 RelayLink 喂给 LinkSource，后台 serve。

use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use p2p_identity::Keypair;
use p2p_mux::{BoxedStream, MuxControl};
use p2p_relay::{LinkSource, RelayLimits, RelayLink, RelayService, RelayServiceImpl};
use p2p_transport::{QuicTransport, TcpTransport};
use tokio::sync::mpsc;

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

/// 从 data 目录加载或生成身份（与 facade 同一 key.seed，确保 PeerId 一致）。
pub fn load_identity(data: PathBuf) -> io::Result<Keypair> {
    std::fs::create_dir_all(&data)?;
    p2p_identity::load_or_generate_seed(&data.join("key.seed"))
}

/// 启动 relay：绑 quic/tcp 监听、两条 accept 循环喂 LinkSource，后台 serve。
/// 返回服务句柄供指标读取；serve 在后台任务里持有另一份克隆。
pub async fn spawn_relay(
    keypair: Keypair,
    quic_addr: SocketAddr,
    tcp_addr: SocketAddr,
) -> io::Result<Arc<RelayServiceImpl>> {
    let (tx, rx) = mpsc::channel::<Box<dyn RelayLink>>(32);
    let source = Box::new(RelayConnSource::new(rx));
    let svc = Arc::new(RelayServiceImpl::new(source, RelayLimits::default()));
    let serve_handle = Arc::clone(&svc);
    tokio::spawn(async move {
        if let Err(e) = RelayService::serve(serve_handle).await {
            tracing::error!(error = %e, "relay service exited with error");
        }
    });

    let quic = QuicTransport::bind(quic_addr, &keypair).await?;
    tokio::spawn(accept_quic_loop(quic, tx.clone()));

    let tcp = TcpTransport::new();
    let listener = tcp.bind(tcp_addr).await?;
    tokio::spawn(accept_tcp_loop(tcp, listener, keypair, tx));

    Ok(svc)
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
