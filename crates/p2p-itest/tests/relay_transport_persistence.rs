//! E4 回归：relay 控制流在真实 QUIC/TCP 传输上静默存活（缺陷 a 转回归）。
//!
//! 缺陷根因：quinn 的 IdleTimeout 底层 VarInt 单位是毫秒，此前把
//! QUIC_IDLE_TIMEOUT.as_secs()=30 直接传入 → 实际空闲超时 30 毫秒；
//! 控制注册交换完头几包后一旦静默即被 quinn 判 TimedOut 杀掉整条连接
//! （日志 connection lost <- timed out），控制流 ~90-120ms 秒断、31/31 必现。
//! 本测试断言：控制注册后静默 2 秒（>> 30ms 缺陷窗口，<< 30s 超时）控制流
//! 仍然存活且可继续控制面往返。

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use p2p_identity::Keypair;
use p2p_itest::expect_within;
use p2p_mux::{BoxedStream, MuxControl};
use p2p_relay::{
    MockLinkSource, RelayClient, RelayEvent, RelayLimits, RelayLink, RelayService, RelayServiceImpl,
};
use p2p_transport::{QuicTransport, TcpTransport, Transport, TransportAddr};

const LIMIT: Duration = Duration::from_secs(10);
/// 静默窗口：缺陷下连接 ~30-120ms 即死；裁决要求 >3s 静默存活，取 4s 留余量。
const IDLE_WINDOW: Duration = Duration::from_secs(4);

/// 单条已认证传输连接 = 一条 RelayLink（peer 取握手互认身份）。
struct ConnLink {
    peer: String,
    mux: Arc<dyn MuxControl>,
}

#[async_trait::async_trait]
impl RelayLink for ConnLink {
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

/// 起中继服务端，返回入站链路注入口。
fn spawn_relay_server() -> MockLinkSource {
    let source = MockLinkSource::new();
    let svc = Arc::new(RelayServiceImpl::new(
        Box::new(source.clone()),
        RelayLimits::default(),
    ));
    tokio::spawn(async move {
        let _ = RelayService::serve(svc).await;
    });
    source
}

/// 断言：注册后静默 IDLE_WINDOW，控制流未断且可再往返。
async fn assert_control_survives_idle(mut client: RelayClient) {
    expect_within(
        "control registration",
        client.reserve(Duration::from_secs(3600), ""),
        LIMIT,
    )
    .await
    .expect("registration reserve");

    let closed = tokio::time::timeout(IDLE_WINDOW, client.next_event()).await;
    match closed {
        Err(_) => {} // 静默窗口内无事件 = 控制流存活
        Ok(Some(RelayEvent::ControlClosed)) => panic!("control stream died during idle window"),
        Ok(other) => panic!("unexpected relay event: {other:?}"),
    }

    // 控制面仍然可用：第二次往返必须成功
    expect_within(
        "post-idle control roundtrip",
        client.reserve(Duration::from_secs(60), ""),
        LIMIT,
    )
    .await
    .expect("control roundtrip after idle");
}

#[tokio::test]
async fn quic_relay_control_survives_idle_window() {
    let source = spawn_relay_server();
    let keypair = Keypair::generate();

    let server = QuicTransport::bind(SocketAddr::from(([127, 0, 0, 1], 0)), &keypair)
        .await
        .expect("quic bind");
    let port = server.local_addr().expect("local addr").port();
    tokio::spawn(async move {
        while let Some(conn) = server.accept().await {
            source.push(Box::new(ConnLink {
                peer: conn.remote.to_string(),
                mux: conn.mux,
            }));
        }
    });

    let client_transport = QuicTransport::new().expect("quic client endpoint");
    let conn = client_transport
        .dial(
            &TransportAddr::Quic {
                ip: std::net::IpAddr::from([127, 0, 0, 1]),
                port,
            },
            &keypair,
            None,
        )
        .await
        .expect("quic dial relay");
    let client = RelayClient::new(Box::new(ConnLink {
        peer: conn.remote.to_string(),
        mux: conn.mux,
    }));
    assert_control_survives_idle(client).await;
}

#[tokio::test]
async fn tcp_relay_control_survives_idle_window() {
    let source = spawn_relay_server();
    let keypair = Keypair::generate();

    let tcp = TcpTransport::new();
    let listener = tcp
        .bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("tcp bind");
    let port = listener.local_addr().expect("local addr").port();
    let server_keypair = keypair.clone();
    tokio::spawn(async move {
        loop {
            match tcp.accept(&listener, &server_keypair).await {
                Ok(conn) => source.push(Box::new(ConnLink {
                    peer: conn.remote.to_string(),
                    mux: conn.mux,
                })),
                Err(e) => {
                    eprintln!("tcp relay accept failed: {e}");
                    break;
                }
            }
        }
    });

    let client_transport = TcpTransport::new();
    let conn = client_transport
        .dial(
            &TransportAddr::Tcp {
                ip: std::net::IpAddr::from([127, 0, 0, 1]),
                port,
            },
            &keypair,
            None,
        )
        .await
        .expect("tcp dial relay");
    let client = RelayClient::new(Box::new(ConnLink {
        peer: conn.remote.to_string(),
        mux: conn.mux,
    }));
    assert_control_survives_idle(client).await;
}
