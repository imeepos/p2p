//! p2p-itest：跨 crate 互操作测试的共享接缝。
//!
//! 只放测试装置：rendezvous duplex 帧化连接、relay 双客户端 harness、有界等待。
//! 断言全部在各 tests/ 用例内；失败必须显式 panic，不允许静默通过。

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use p2p_discovery::rendezvous::{RendezvousConn, RendezvousError, RendezvousLink};
use p2p_relay::{MockLinkSource, RelayClient, RelayLimits, RelayService, RelayServiceImpl};
use tokio::sync::mpsc;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

/// 把任意字节流按 rendezvous 线格式（u32 大端长度前缀 + prost 负载）帧化。
/// 与 p2p-discovery 内部 mock 同一格式，供外部会话经 duplex 对接 serve_link。
pub fn rendezvous_conn<S>(io: S) -> RendezvousConn
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (read_half, write_half) = tokio::io::split(io);
    let (out_tx, mut out_rx) = mpsc::channel::<Vec<u8>>(16);
    tokio::spawn(async move {
        let mut framed = FramedWrite::new(write_half, LengthDelimitedCodec::new());
        while let Some(frame) = out_rx.recv().await {
            if framed.send(bytes::Bytes::from(frame)).await.is_err() {
                break;
            }
        }
        let _ = framed.close().await;
    });
    let (in_tx, in_rx) = mpsc::channel::<Result<Vec<u8>, RendezvousError>>(16);
    tokio::spawn(async move {
        let mut framed = FramedRead::new(read_half, LengthDelimitedCodec::new());
        while let Some(item) = framed.next().await {
            let mapped = item
                .map(|frame| frame.to_vec())
                .map_err(|e| RendezvousError::Link(e.to_string()));
            if in_tx.send(mapped).await.is_err() {
                break;
            }
        }
    });
    let read = Box::pin(futures::stream::unfold(in_rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    }));
    RendezvousConn {
        write: out_tx,
        read,
    }
}

/// 测试用 RendezvousLink：预置的 duplex 即一次传输连接，取用一次即耗尽。
pub struct SingleDuplexLink(pub tokio::sync::Mutex<Option<tokio::io::DuplexStream>>);

#[async_trait::async_trait]
impl RendezvousLink for SingleDuplexLink {
    async fn connect(&self) -> Result<RendezvousConn, RendezvousError> {
        let io = self
            .0
            .lock()
            .await
            .take()
            .ok_or_else(|| RendezvousError::Link("single duplex already consumed".into()))?;
        Ok(rendezvous_conn(io))
    }
}

/// 进程内 relay：接好 a/b 两条 mock duplex 链路并后台 serve，返回两侧客户端。
pub fn relay_pair(limits: RelayLimits, peer_a: &str, peer_b: &str) -> (RelayClient, RelayClient) {
    let source = MockLinkSource::new();
    let (client_a, server_a) = p2p_relay::mock_link_pair(peer_a, "relay");
    let (client_b, server_b) = p2p_relay::mock_link_pair(peer_b, "relay");
    source.push(Box::new(server_a));
    source.push(Box::new(server_b));
    let svc = Arc::new(RelayServiceImpl::new(Box::new(source), limits));
    tokio::spawn(async move {
        let _ = RelayService::serve(svc).await;
    });
    (
        RelayClient::new(Box::new(client_a)),
        RelayClient::new(Box::new(client_b)),
    )
}

/// 有界等待：超时即 panic，测试不允许静默悬挂。
pub async fn expect_within<F: Future>(what: &str, fut: F, limit: Duration) -> F::Output {
    match tokio::time::timeout(limit, fut).await {
        Ok(value) => value,
        Err(_) => panic!("timed out waiting for {what}"),
    }
}
