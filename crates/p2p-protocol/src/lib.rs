//! 协议分发层契约（design §5）。
//!
//! 帧格式：varint(len) + payload，单帧上限 [MAX_FRAME_SIZE]；
//! 新开流的首帧为协议 ID 的 UTF-8 字节（同样走帧封装）。
//! ProtocolId/帧编解码/注册表为冻结契约；request-response、StreamFactory 接缝、
//! 协议握手助手与 chunked transfer 由协议会话 P 在此只增不改地实现。

use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use p2p_identity::PeerId;
use p2p_mux::BoxedStream;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

mod chunked;
mod handshake;
mod request_response;
mod stream_factory;

pub use chunked::{
    read_chunked, write_chunked, CHUNK_DATA_SIZE, FRAME_CHUNK, FRAME_END, FRAME_SINGLE,
    MAX_MESSAGE_SIZE,
};
pub use handshake::{dispatch_inbound, dispatch_inbound_with_peer, open_with_protocol};
pub use request_response::RequestResponseClient;
pub use stream_factory::{LoopbackHub, StreamFactory};

pub const MAX_FRAME_SIZE: u32 = 1 << 20;

/// 协议 ID：`/<seg>/<seg>/<n>`，版本段为纯数字，如 `/p2p-base/rendezvous/1`。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProtocolId(String);

impl ProtocolId {
    pub fn new(s: &str) -> Result<Self, ProtocolError> {
        let invalid = || ProtocolError::InvalidId(s.to_string());
        let body = s.strip_prefix('/').ok_or_else(invalid)?;
        let segs: Vec<&str> = body.split('/').collect();
        if segs.len() < 2 {
            return Err(invalid());
        }
        let version = segs[segs.len() - 1];
        if version.is_empty() || !version.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid());
        }
        if !segs[..segs.len() - 1].iter().all(|n| is_segment(n)) {
            return Err(invalid());
        }
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_segment(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

impl std::fmt::Display for ProtocolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("invalid protocol id: {0}")]
    InvalidId(String),
    #[error("frame too large: {0} bytes (max {MAX_FRAME_SIZE})")]
    FrameTooLarge(u64),
    #[error("chunked message too large: {0} bytes")]
    MessageTooLarge(u64),
    #[error("request timed out after {0:?}")]
    Timeout(std::time::Duration),
    #[error("protocol not supported by remote handler: {0}")]
    UnsupportedProtocol(ProtocolId),
    #[error("io: {0}")]
    Io(#[from] io::Error),
}

/// 把「io::Error 包装的 ProtocolError」还原为顶层 ProtocolError，保持错误可区分。
/// 非 ProtocolError 载荷或无载荷时保留原 kind 包回 Io。
pub fn flatten_io(err: io::Error) -> ProtocolError {
    let kind = err.kind();
    match err.into_inner() {
        Some(inner) => match inner.downcast::<ProtocolError>() {
            Ok(pe) => *pe,
            Err(inner) => ProtocolError::Io(io::Error::new(kind, inner)),
        },
        None => ProtocolError::Io(io::Error::new(kind, "no error payload")),
    }
}

/// 业务接入点：注册协议 ID，收到对应流即被回调（design §9）。
/// handler 拥有该流直到返回，返回即关流。
///
/// 入站流的对端身份由分发层随流下传（libp2p 同类痛点通行解：trait 加参）：
/// 分发层已从安全握手互认 PeerId（SecureConn.remote），handler 无需再靠
/// 「在线集推断」绕行归属。swarm 分发统一走 [Self::handle_inbound]；
/// 旧接缝 [Self::handle] 保留给无身份上下文的裸流场景（盲拨应答等）。
#[async_trait::async_trait]
pub trait ProtocolHandler: Send + Sync {
    fn protocol(&self) -> ProtocolId;

    /// 已知对端身份的入站分发入口（swarm serve 路径唯一入口）。
    /// 默认桥接旧签名：未升级 handler 行为不变，需要真实身份时覆写本方法。
    async fn handle_inbound(&self, peer: PeerId, stream: BoxedStream) -> io::Result<()> {
        let _ = peer;
        self.handle(stream).await
    }

    /// 裸流入站（无对端身份上下文）：盲拨应答、纯回环测试等场景。
    async fn handle(&self, stream: BoxedStream) -> io::Result<()>;
}

/// 协议 ID → handler 路由表。
#[derive(Default)]
pub struct HandlerRegistry {
    handlers: HashMap<ProtocolId, Arc<dyn ProtocolHandler>>,
}

impl HandlerRegistry {
    pub fn register(&mut self, handler: Arc<dyn ProtocolHandler>) {
        let id = handler.protocol();
        self.handlers.insert(id, handler);
    }

    pub fn get(&self, id: &ProtocolId) -> Option<Arc<dyn ProtocolHandler>> {
        self.handlers.get(id).cloned()
    }

    pub fn protocols(&self) -> Vec<ProtocolId> {
        self.handlers.keys().cloned().collect()
    }
}

pub async fn write_frame(
    w: &mut (impl AsyncWrite + Unpin + Send),
    payload: &[u8],
) -> io::Result<()> {
    let len = payload.len() as u64;
    if len > MAX_FRAME_SIZE as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            ProtocolError::FrameTooLarge(len),
        ));
    }
    write_varint(w, len).await?;
    w.write_all(payload).await
}

pub async fn read_frame(r: &mut (impl AsyncRead + Unpin + Send)) -> io::Result<Vec<u8>> {
    let len = read_varint(r).await?;
    if len > MAX_FRAME_SIZE as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            ProtocolError::FrameTooLarge(len),
        ));
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf).await?;
    Ok(buf)
}

pub async fn write_protocol_id(
    w: &mut (impl AsyncWrite + Unpin + Send),
    id: &ProtocolId,
) -> io::Result<()> {
    write_frame(w, id.as_str().as_bytes()).await
}

pub async fn read_protocol_id(r: &mut (impl AsyncRead + Unpin + Send)) -> io::Result<ProtocolId> {
    let bytes = read_frame(r).await?;
    let s = String::from_utf8(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "protocol id not utf-8"))?;
    ProtocolId::new(&s).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

async fn write_varint(w: &mut (impl AsyncWrite + Unpin + Send), mut v: u64) -> io::Result<()> {
    let mut buf = [0u8; 10];
    let mut i = 0;
    loop {
        let mut byte = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        buf[i] = byte;
        i += 1;
        if v == 0 {
            break;
        }
    }
    w.write_all(&buf[..i]).await
}

async fn read_varint(r: &mut (impl AsyncRead + Unpin + Send)) -> io::Result<u64> {
    let mut v: u64 = 0;
    let mut shift: u32 = 0;
    loop {
        let mut byte = [0u8; 1];
        r.read_exact(&mut byte).await?;
        // 第 10 字节（shift=63）只剩 1 个有效数据位：
        // 数据位 > 1 或仍带继续位都会超出 64 位，拒绝而不是静默回绕。
        if shift >= 64 || (shift == 63 && (byte[0] & 0x7f) > 0x01) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "varint overflow",
            ));
        }
        v |= u64::from(byte[0] & 0x7f) << shift;
        if byte[0] & 0x80 == 0 {
            return Ok(v);
        }
        shift += 7;
    }
}

/// request-response 便捷原语接缝（P 会话实现）：
/// open 负责从连接池产出到目标 peer 的已开流（含协议 ID 握手）。
#[async_trait::async_trait]
pub trait RequestResponse: Send + Sync {
    async fn request(
        &self,
        peer: PeerId,
        id: ProtocolId,
        payload: Vec<u8>,
        timeout: std::time::Duration,
    ) -> Result<Vec<u8>, ProtocolError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_id_validation() {
        assert!(ProtocolId::new("/p2p-base/rendezvous/1").is_ok());
        assert!(ProtocolId::new("/myapp/chat/2").is_ok());
        assert!(ProtocolId::new("p2p-base/rendezvous/1").is_err());
        assert!(ProtocolId::new("/p2p-base/rendezvous").is_err());
        assert!(ProtocolId::new("/p2p-base/rendezvous/v1").is_err());
        assert!(ProtocolId::new("/p2p base/rendezvous/1").is_err());
    }

    #[tokio::test]
    async fn frame_roundtrip_and_limits() {
        let (mut tx, mut rx) = tokio::io::duplex(1024);
        let payload = b"hello frame".to_vec();
        let (w, r) = tokio::join!(write_frame(&mut tx, &payload), read_frame(&mut rx));
        w.unwrap();
        assert_eq!(r.unwrap(), payload);
    }

    #[tokio::test]
    async fn varint_ten_byte_boundary_roundtrip() {
        let (mut tx, mut rx) = tokio::io::duplex(64);
        let (w, r) = tokio::join!(write_varint(&mut tx, u64::MAX), read_varint(&mut rx));
        w.unwrap();
        assert_eq!(r.unwrap(), u64::MAX);
    }

    #[tokio::test]
    async fn varint_overflow_input_rejected() {
        let mut high_bit = [0xffu8; 10];
        high_bit[9] = 0x02; // 第 10 字节不带继续位，但数据位 > 1（旧实现静默回绕）
        let continues = [0xffu8; 10]; // 第 10 字节仍带继续位
        for bytes in [high_bit, continues] {
            let mut cursor = &bytes[..];
            let err = read_varint(&mut cursor).await.unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        }
    }
}
