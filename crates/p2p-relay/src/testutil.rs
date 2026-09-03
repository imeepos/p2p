//! 测试替身（E9-Q0 审计 2.3 收口）：duplex mock 链路与可注入链路源。
//!
//! 仅供测试消费（relay 自身回归 / itest / facade 测试夹具）；产线代码
//! 禁止使用——真实链路接缝是 [crate::link] 的 RelayLink/LinkSource。

use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use p2p_mux::BoxedStream;
use tokio::sync::{mpsc, Mutex};

use crate::link::{LinkSource, RelayLink};

/// 单条 mock 流的 duplex 缓冲（字节），限制单流积压。
pub const STREAM_BUF: usize = 4096;

/// 测试/装配接缝：open 与对端 accept 交叉相连的 mock 链路。
pub struct MockLink {
    peer: String,
    open_tx: mpsc::Sender<BoxedStream>,
    accept_rx: Mutex<mpsc::Receiver<BoxedStream>>,
}

/// 生成一对互相连通的 mock 链路：a 开流由 b 收，b 开流由 a 收。
pub fn mock_link_pair(peer_a: &str, peer_b: &str) -> (MockLink, MockLink) {
    let (tx_ab, rx_ab) = mpsc::channel(64);
    let (tx_ba, rx_ba) = mpsc::channel(64);
    let a = MockLink {
        peer: peer_a.into(),
        open_tx: tx_ab,
        accept_rx: Mutex::new(rx_ba),
    };
    let b = MockLink {
        peer: peer_b.into(),
        open_tx: tx_ba,
        accept_rx: Mutex::new(rx_ab),
    };
    (a, b)
}

#[async_trait]
impl RelayLink for MockLink {
    fn peer_id(&self) -> &str {
        &self.peer
    }

    async fn open_stream(&self) -> io::Result<BoxedStream> {
        let (ours, theirs) = tokio::io::duplex(STREAM_BUF);
        self.open_tx
            .send(Box::new(theirs))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "link peer gone"))?;
        Ok(Box::new(ours))
    }

    async fn accept_stream(&self) -> Option<BoxedStream> {
        self.accept_rx.lock().await.recv().await
    }
}

struct MockSourceInner {
    tx: mpsc::UnboundedSender<Box<dyn RelayLink>>,
    rx: Mutex<mpsc::UnboundedReceiver<Box<dyn RelayLink>>>,
}

/// 可克隆的链路源：push 注入；读空时挂起，全部发送端归还后返回 None。
#[derive(Clone)]
pub struct MockLinkSource {
    inner: Arc<MockSourceInner>,
}

impl Default for MockLinkSource {
    fn default() -> Self {
        Self::new()
    }
}

impl MockLinkSource {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            inner: Arc::new(MockSourceInner {
                tx,
                rx: Mutex::new(rx),
            }),
        }
    }

    pub fn push(&self, link: Box<dyn RelayLink>) {
        let _ = self.inner.tx.send(link);
    }
}

#[async_trait]
impl LinkSource for MockLinkSource {
    async fn next_link(&self) -> Option<Box<dyn RelayLink>> {
        self.inner.rx.lock().await.recv().await
    }
}
