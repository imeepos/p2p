//! 连接缝：RendezvousLink 产出 BoxedStream 帧流，客户端/服务端只对该 seam 编程，不依赖真实传输。
//! 真实传输（QUIC/TCP）由编排会话提供该 trait 的实现；测试用 tokio duplex mock。

use futures::stream::BoxStream;
use futures::StreamExt;
use tokio::sync::mpsc;

use crate::rendezvous::messages::{Request, Response};

#[derive(Debug, thiserror::Error)]
pub enum RendezvousError {
    #[error("link: {0}")]
    Link(String),
    #[error("send: {0}")]
    Send(String),
    #[error("protocol: {0}")]
    Protocol(String),
    /// 读端干净收尾（对端查询即断、会话正常结束）：服务端不计为错误，
    /// 客户端视为链路终止照常走重连。
    #[error("link closed")]
    Closed,
}

/// 一条已建立的双向帧流：write 发送完整帧（长度前缀），read 为接收帧的 BoxedStream。
pub struct RendezvousConn {
    pub write: mpsc::Sender<Vec<u8>>,
    pub read: BoxStream<'static, Result<Vec<u8>, RendezvousError>>,
}

impl RendezvousConn {
    pub async fn send_msg<M: prost::Message>(&mut self, msg: &M) -> Result<(), RendezvousError> {
        let mut buf = Vec::new();
        msg.encode(&mut buf)
            .map_err(|e| RendezvousError::Send(e.to_string()))?;
        self.write
            .send(buf)
            .await
            .map_err(|_| RendezvousError::Send("write half closed".into()))
    }

    /// 发送已编码帧（快照缓存路径：应答只编码一次，命中期免重复 encode）。
    pub async fn send_raw(&mut self, bytes: Vec<u8>) -> Result<(), RendezvousError> {
        self.write
            .send(bytes)
            .await
            .map_err(|_| RendezvousError::Send("write half closed".into()))
    }

    pub async fn recv_msg<M: prost::Message + Default>(&mut self) -> Result<M, RendezvousError> {
        let bytes = self.read.next().await.ok_or(RendezvousError::Closed)??;
        M::decode(bytes.as_slice()).map_err(|e| RendezvousError::Protocol(e.to_string()))
    }

    /// 请求-应答一趟：发送 Request，等待 Response。
    pub async fn roundtrip(&mut self, req: Request) -> Result<Response, RendezvousError> {
        self.send_msg(&req).await?;
        self.recv_msg::<Response>().await
    }
}

/// 发现源与 rendezvous 服务器之间的连接缝：connect 产出可用帧流。
#[async_trait::async_trait]
pub trait RendezvousLink: Send + Sync {
    async fn connect(&self) -> Result<RendezvousConn, RendezvousError>;
}

#[cfg(test)]
pub(crate) mod mock {
    use super::*;
    use bytes::Bytes;
    use futures::stream::unfold;
    use futures::SinkExt;
    use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

    /// 从 tokio duplex 构造帧流连接（长度前缀 + prost 负载），测试专用。
    pub(crate) fn conn_from_duplex(duplex: tokio::io::DuplexStream) -> RendezvousConn {
        let (read_half, write_half) = tokio::io::split(duplex);
        let (out_tx, mut out_rx) = mpsc::channel::<Vec<u8>>(16);
        tokio::spawn(async move {
            let mut framed = FramedWrite::new(write_half, LengthDelimitedCodec::new());
            while let Some(frame) = out_rx.recv().await {
                if framed.send(Bytes::from(frame)).await.is_err() {
                    break;
                }
            }
            let _ = framed.close().await;
        });
        let (in_tx, in_rx) = mpsc::channel::<Result<Vec<u8>, RendezvousError>>(16);
        tokio::spawn(async move {
            let mut framed = FramedRead::new(read_half, LengthDelimitedCodec::new());
            while let Some(item) = framed.next().await {
                match item {
                    Ok(frame) => {
                        if in_tx.send(Ok(frame.to_vec())).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = in_tx.send(Err(RendezvousError::Link(e.to_string()))).await;
                        break;
                    }
                }
            }
        });
        let read = Box::pin(unfold(in_rx, |mut rx| async move {
            rx.recv().await.map(|item| (item, rx))
        }));
        RendezvousConn {
            write: out_tx,
            read,
        }
    }

    /// 测试用 link 实现：把预置的 duplex 当一次传输连接。
    pub(crate) struct MockLink {
        inner: tokio::sync::Mutex<Option<tokio::io::DuplexStream>>,
    }

    impl MockLink {
        pub(crate) fn new(duplex: tokio::io::DuplexStream) -> Self {
            Self {
                inner: tokio::sync::Mutex::new(Some(duplex)),
            }
        }
    }

    #[async_trait::async_trait]
    impl RendezvousLink for MockLink {
        async fn connect(&self) -> Result<RendezvousConn, RendezvousError> {
            let duplex = self
                .inner
                .lock()
                .await
                .take()
                .ok_or_else(|| RendezvousError::Link("mock link already connected".into()))?;
            Ok(conn_from_duplex(duplex))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::conn_from_duplex;
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn duplex_frame_roundtrip() {
        let (client, peer) = tokio::io::duplex(2048);
        let mut conn = conn_from_duplex(client);
        let peer_task = tokio::spawn(async move {
            use tokio_util::codec::{FramedRead, LengthDelimitedCodec};
            let (peer_r, mut peer_w) = tokio::io::split(peer);
            let mut framed = FramedRead::new(peer_r, LengthDelimitedCodec::new());
            let frame = framed.next().await.expect("frame").expect("io");
            let data = frame.to_vec();
            assert_eq!(data, b"hello-frame".to_vec());
            let mut buf = Vec::with_capacity(4 + data.len());
            buf.extend_from_slice(&(data.len() as u32).to_be_bytes());
            buf.extend_from_slice(&data);
            peer_w.write_all(&buf).await.expect("write");
        });
        conn.write
            .send(b"hello-frame".to_vec())
            .await
            .expect("send");
        let recv = conn.read.next().await.expect("frame").expect("io");
        assert_eq!(recv, b"hello-frame".to_vec());
        peer_task.await.expect("peer task");
    }
}
