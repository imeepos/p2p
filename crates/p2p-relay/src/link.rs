//! 链路接缝：对真实 transport（K 会话）的依赖点；测试用 tokio duplex mock。
//!
//! RelayLink 抽象「一条已连到 relay 的连接」，可开/收多条 BoxedStream：
//! 真实实现挂 mux 流，mock 用 tokio::io::duplex 两端交叉相连。

use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use p2p_mux::BoxedStream;
use tokio::sync::{mpsc, Mutex};

/// 单条 mock 流的 duplex 缓冲（字节），限制单流积压。
pub const STREAM_BUF: usize = 4096;

#[async_trait]
pub trait RelayLink: Send + Sync {
    /// 已认证对端标识（transport 握手后注入；服务端据此做配额）。
    fn peer_id(&self) -> &str;
    /// 开一条新流：控制面 Reserve/Connect 或电路数据接续。
    async fn open_stream(&self) -> io::Result<BoxedStream>;
    /// 收对端开来的下一条流；None = 链路已关闭。
    async fn accept_stream(&self) -> Option<BoxedStream>;
}

/// 入站链路来源；发送端全部归还后 next_link 返回 None，服务优雅退出。
#[async_trait]
pub trait LinkSource: Send + Sync {
    async fn next_link(&self) -> Option<Box<dyn RelayLink>>;
}

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
    let a = MockLink { peer: peer_a.into(), open_tx: tx_ab, accept_rx: Mutex::new(rx_ba) };
    let b = MockLink { peer: peer_b.into(), open_tx: tx_ba, accept_rx: Mutex::new(rx_ab) };
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
        Self { inner: Arc::new(MockSourceInner { tx, rx: Mutex::new(rx) }) }
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
