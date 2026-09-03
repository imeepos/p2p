//! 字节流与多路复用抽象。
//!
//! QUIC 用原生多流，TCP 挂 yamux，均实现为 [MuxControl]（实现归内核会话）。

use std::{io, sync::Arc};

/// 双向字节流的统一对象安全别名：TCP/QUIC/Noise/中继电路的产物均可装入。
pub trait ByteStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send> ByteStream for T {}

pub type BoxedStream = Box<dyn ByteStream>;

/// 复用控制器：在一条已认证连接上开/收多条互相独立的逻辑流。
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

/// 每连接并发流数上限（design §6 通信层防滥用）。
/// yamux 侧同步写入协议配置；QUIC 侧叠加在 quinn 传输参数之上。
pub const MAX_STREAMS_PER_CONN: usize = 64;
