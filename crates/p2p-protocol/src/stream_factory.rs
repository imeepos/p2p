//! StreamFactory 接缝：swarm 侧将来接连接池/拨号器，本 crate 只按 (peer, 协议) 要流。
//!
//! 返回的是已开启的裸流，停在协议握手之前：协议 ID 首帧由
//! [open_with_protocol](crate::open_with_protocol) 或 request-response 写入，
//! 工厂不重复写（design §5.1：每条流第一帧即协议 ID）。

use std::io;
use std::sync::Arc;

use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use tokio::io::duplex;
use tokio::sync::mpsc;

use crate::ProtocolId;

/// 产出「已开流」的抽象：实现方负责拨号 peer 并在既有连接上开一条新流。
#[async_trait::async_trait]
pub trait StreamFactory: Send + Sync {
    /// 拨向 peer 并产出一条裸流；协议握手由调用方完成。
    async fn open_stream(&self, peer: &PeerId, protocol: &ProtocolId) -> io::Result<BoxedStream>;
}

/// 进程内 mock：每对连接用 tokio duplex 造流，对端一侧进入 hub 收流队列，
/// 模拟「对端收到新开流」。swarm 真实接线前的测试接缝。
#[derive(Clone)]
pub struct LoopbackHub {
    inbound: Arc<mpsc::Sender<BoxedStream>>,
    buf: usize,
}

impl LoopbackHub {
    /// buf 为每对 duplex 的缓冲字节数；receiver 按顺序收到每条流的对端侧。
    pub fn new(queue_cap: usize, buf: usize) -> (Self, mpsc::Receiver<BoxedStream>) {
        let (tx, rx) = mpsc::channel(queue_cap);
        (
            Self {
                inbound: Arc::new(tx),
                buf,
            },
            rx,
        )
    }
}

#[async_trait::async_trait]
impl StreamFactory for LoopbackHub {
    async fn open_stream(&self, _peer: &PeerId, _protocol: &ProtocolId) -> io::Result<BoxedStream> {
        let (client, server) = duplex(self.buf);
        self.inbound.send(Box::new(server)).await.map_err(|_| {
            io::Error::new(io::ErrorKind::BrokenPipe, "loopback inbound queue closed")
        })?;
        Ok(Box::new(client))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProtocolId;

    #[tokio::test]
    async fn loopback_yields_paired_stream() {
        let id = ProtocolId::new("/test/loop/1").unwrap();
        let (hub, mut inbound) = LoopbackHub::new(4, 1024);
        let mut client = hub
            .open_stream(&PeerId::from_bytes([1; 32]), &id)
            .await
            .unwrap();
        let mut server = inbound.recv().await.unwrap();
        tokio::io::AsyncWriteExt::write_all(&mut client, b"ping")
            .await
            .unwrap();
        let mut buf = vec![0u8; 4];
        tokio::io::AsyncReadExt::read_exact(&mut server, &mut buf)
            .await
            .unwrap();
        assert_eq!(buf, b"ping");
    }
}
