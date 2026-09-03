//! 链路接缝：对真实 transport（K 会话）的依赖点。
//!
//! RelayLink 抽象「一条已连到 relay 的连接」，可开/收多条 BoxedStream：
//! 真实实现挂 mux 流；测试用 duplex mock 见 [crate::testutil]。

use std::io;

use async_trait::async_trait;
use p2p_mux::BoxedStream;

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
