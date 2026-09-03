//! 字节流与多路复用抽象。
//!
//! QUIC 用原生多流，TCP 挂 yamux，均实现为 [MuxControl]（实现归内核会话）。

use std::{io, sync::Arc};

/// 双向字节流的统一对象安全别名：TCP/QUIC/Noise/中继电路的产物均可装入。
pub trait ByteStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send> ByteStream for T {}

pub type BoxedStream = Box<dyn ByteStream>;

/// 复用控制器：在一条已认证连接上开/收多条互相独立的逻辑流。
///
/// 生命周期统一语义（E8-H3 定稿，对照全文见 docs/design/mux-transport-lifecycle.md）：
///
/// - 连接终止只认三类事件：本端显式 [close](Self::close)、对端关闭、
///   传输层错误/空闲超时；
/// - close() 幂等：已关闭连接上再调用无副作用；之后 open_stream 以错误
///   收敛，本端与对端 accept_stream 随连接收敛返回 None；
/// - 句柄全部丢弃不是契约语义的关闭：过渡期实现差异（YamuxMux 归零即终、
///   QuicMux 由 quinn 驱动与活跃流维持至空闲超时）已登记待裁决，上层不得
///   依赖任一方向的丢弃行为；
/// - 读半结束两级：流级对端 FIN 以 0 字节读（EOF）呈现；会话级终止令
///   accept_stream 返回 None。两实现一致。
#[async_trait::async_trait]
pub trait MuxControl: Send + Sync {
    /// 主动开一条逻辑流。实现必须施加每连接流数上限（防滥用，design §6）。
    async fn open_stream(&self) -> io::Result<BoxedStream>;

    /// 接收对端开来的下一条逻辑流；连接关闭后返回 None。
    async fn accept_stream(&self) -> Option<BoxedStream>;

    /// 本端主动关闭整条连接（挂断）；对端 accept_stream 随之返回 None。
    /// 幂等：已关闭的连接上再调用必须无副作用。
    fn close(&self);
}

/// 装箱为对象安全的复用句柄。
pub fn boxed_mux(m: impl MuxControl + 'static) -> Arc<dyn MuxControl> {
    Arc::new(m)
}
mod limited;
mod quic_mux;
mod yamux_mux;

pub use quic_mux::QuicMux;
pub use yamux_mux::YamuxMux;

/// E7-K2 错误链载体：std 的 `io::Error::source()` 只返回载荷自身的 source
/// 而不返回载荷，直接 `io::Error::new(kind, inner)` 会令 source() 遍历盲视内层。
/// 以本包装器作载荷：`err.to_string()` 保持内层原文，`err.source()` 即内层错误，
/// 调用方沿 source 链 downcast 可还原内层类型（E5 登记项修复的基础设施）。
#[derive(Debug)]
pub(crate) struct ChainedPayload<E> {
    inner: E,
}

impl<E: std::fmt::Display> std::fmt::Display for ChainedPayload<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ChainedPayload<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.inner)
    }
}

/// 每连接并发流数上限（design §6 通信层防滥用）。
/// yamux 侧同步写入协议配置；QUIC 侧叠加在 quinn 传输参数之上。
pub const MAX_STREAMS_PER_CONN: usize = 64;
