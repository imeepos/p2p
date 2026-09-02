//! relay 控制面消息（design 5.3/5.4）：prost derive 手写，免 protoc。
//!
//! 线格式：varint 长度前缀 + RelayMsg protobuf 帧（design 5.2，单帧上限 1 MiB）。
//! Punch 信令语义：外发消息的 peer_id 指明目的地；relay 转发时改写为发送方，
//! 接收方看到的 peer_id 即对端真实身份。

use prost::Message as ProstMessage;
use std::io::{Error, ErrorKind};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// 服务端错误码（Reject.code），与 crate::error::RelayError 互映。
pub mod errcode {
    /// connect 引用的电路不存在。
    pub const UNKNOWN_CIRCUIT: u32 = 1;
    /// 电路预留已过 TTL。
    pub const CIRCUIT_EXPIRED: u32 = 2;
    /// 每 Peer 配额（链路数/电路数/带宽）超限。
    pub const PEER_LIMIT: u32 = 3;
    /// 协议违规（首帧/状态机不合法）。
    pub const PROTOCOL: u32 = 4;
    /// 打洞目标当前无控制链路可转发。
    pub const PUNCH_TARGET_UNKNOWN: u32 = 5;
    /// 接入方不在电路允许名单（审查 M2）。
    pub const FORBIDDEN_JOINER: u32 = 6;
    /// 全站资源总量已打满（审查 M5）。
    pub const GLOBAL_CAPACITY: u32 = 7;
}

/// 申请中继电路；ttl_secs=0 表示使用服务端缺省 TTL。
/// allowed_joiner 为空 = 仅 reserve 者可接入，否则只允许该 PeerId 接入。
#[derive(Clone, PartialEq, prost::Message)]
pub struct Reserve {
    #[prost(uint64, tag = "1")]
    pub ttl_secs: u64,
    #[prost(string, tag = "2")]
    pub allowed_joiner: String,
}

/// 服务端发放电路标识。
#[derive(Clone, PartialEq, prost::Message)]
pub struct Reserved {
    #[prost(uint64, tag = "1")]
    pub circuit_id: u64,
}

/// 接入电路：同号第二条连接到达即开始桥接。
#[derive(Clone, PartialEq, prost::Message)]
pub struct Connect {
    #[prost(uint64, tag = "1")]
    pub circuit_id: u64,
}

/// 电路接入成功，此流后续字节与对端互通。
#[derive(Clone, PartialEq, prost::Message)]
pub struct Bound {
    #[prost(uint64, tag = "1")]
    pub circuit_id: u64,
}

/// 打洞请求（类 DCUtR，design 7.3）：请求对方与自己同时开洞探测。
#[derive(Clone, PartialEq, prost::Message)]
pub struct PunchReq {
    /// 目的地（relay 转发时改写为请求方）。
    #[prost(string, tag = "1")]
    pub peer_id: String,
    /// 发送方观测到的自身地址（host:port），供对方定向探测。
    #[prost(string, repeated, tag = "2")]
    pub addrs: Vec<String>,
}

/// 打洞应答：被动方确认，双方收到即同时探测。
#[derive(Clone, PartialEq, prost::Message)]
pub struct PunchAck {
    /// 目的地（relay 转发时改写为应答方）。
    #[prost(string, tag = "1")]
    pub peer_id: String,
    /// 发送方观测到的自身地址。
    #[prost(string, repeated, tag = "2")]
    pub addrs: Vec<String>,
}

/// 服务端拒绝帧：控制面/接入流上的显式错误信号。
#[derive(Clone, PartialEq, prost::Message)]
pub struct Reject {
    #[prost(uint32, tag = "1")]
    pub code: u32,
    #[prost(string, tag = "2")]
    pub message: String,
}

/// 控制面信封：一帧一个 RelayMsg，kind 只装一个子消息。
#[derive(Clone, PartialEq, prost::Message)]
pub struct RelayMsg {
    #[prost(oneof = "relay_msg::Kind", tags = "1, 2, 3, 4, 5, 6, 7")]
    pub kind: Option<relay_msg::Kind>,
}

/// 信封载荷。
pub mod relay_msg {
    /// 七种控制面帧。
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Kind {
        #[prost(message, tag = "1")]
        Reserve(super::Reserve),
        #[prost(message, tag = "2")]
        Reserved(super::Reserved),
        #[prost(message, tag = "3")]
        Connect(super::Connect),
        #[prost(message, tag = "4")]
        Bound(super::Bound),
        #[prost(message, tag = "5")]
        PunchReq(super::PunchReq),
        #[prost(message, tag = "6")]
        PunchAck(super::PunchAck),
        #[prost(message, tag = "7")]
        Reject(super::Reject),
    }
}

impl RelayMsg {
    pub fn reserve(ttl_secs: u64, allowed_joiner: &str) -> Self {
        Self {
            kind: Some(relay_msg::Kind::Reserve(Reserve {
                ttl_secs,
                allowed_joiner: allowed_joiner.into(),
            })),
        }
    }

    pub fn reserved(circuit_id: u64) -> Self {
        Self {
            kind: Some(relay_msg::Kind::Reserved(Reserved { circuit_id })),
        }
    }

    pub fn connect(circuit_id: u64) -> Self {
        Self {
            kind: Some(relay_msg::Kind::Connect(Connect { circuit_id })),
        }
    }

    pub fn bound(circuit_id: u64) -> Self {
        Self {
            kind: Some(relay_msg::Kind::Bound(Bound { circuit_id })),
        }
    }

    pub fn punch_req(peer_id: impl Into<String>, addrs: Vec<String>) -> Self {
        Self {
            kind: Some(relay_msg::Kind::PunchReq(PunchReq {
                peer_id: peer_id.into(),
                addrs,
            })),
        }
    }

    pub fn punch_ack(peer_id: impl Into<String>, addrs: Vec<String>) -> Self {
        Self {
            kind: Some(relay_msg::Kind::PunchAck(PunchAck {
                peer_id: peer_id.into(),
                addrs,
            })),
        }
    }

    pub fn error(code: u32, message: impl Into<String>) -> Self {
        Self {
            kind: Some(relay_msg::Kind::Reject(Reject {
                code,
                message: message.into(),
            })),
        }
    }
}

/// 单帧上限（design 5.2 缺省 1 MiB）。
pub const MAX_FRAME: usize = 1 << 20;

fn push_varint(buf: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            buf.push(b);
            return;
        }
        buf.push(b | 0x80);
    }
}

/// 读无符号 varint；流在帧边界干净关闭时返回 None。
async fn read_varint(r: &mut (impl AsyncRead + Unpin)) -> std::io::Result<Option<u64>> {
    let mut v = 0u64;
    let mut shift = 0u32;
    loop {
        let b = match r.read_u8().await {
            Ok(b) => b,
            Err(e) if e.kind() == ErrorKind::UnexpectedEof && shift == 0 => return Ok(None),
            Err(e) => return Err(e),
        };
        v |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Ok(Some(v));
        }
        shift += 7;
        if shift >= 64 {
            return Err(Error::new(ErrorKind::InvalidData, "varint overflow"));
        }
    }
}

/// 编码并写一帧。
pub async fn write_msg(w: &mut (impl AsyncWrite + Unpin), msg: &RelayMsg) -> std::io::Result<()> {
    let body = msg.encode_to_vec();
    if body.len() > MAX_FRAME {
        return Err(Error::new(ErrorKind::InvalidData, "relay frame too large"));
    }
    let mut buf = Vec::with_capacity(body.len() + 5);
    push_varint(&mut buf, body.len() as u64);
    buf.extend_from_slice(&body);
    w.write_all(&buf).await
}

/// 读一帧；None = 对端干净关流。
pub async fn read_msg(r: &mut (impl AsyncRead + Unpin)) -> std::io::Result<Option<RelayMsg>> {
    let Some(len) = read_varint(r).await? else {
        return Ok(None);
    };
    if len as usize > MAX_FRAME {
        return Err(Error::new(ErrorKind::InvalidData, "relay frame too large"));
    }
    let mut body = vec![0u8; len as usize];
    r.read_exact(&mut body).await?;
    RelayMsg::decode(&body[..])
        .map(Some)
        .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))
}

/// 在流上写一条显式拒绝帧（失败路径必须留信号，不许静默）。
pub async fn write_reject(
    w: &mut (impl AsyncWrite + Unpin),
    code: u32,
    message: impl Into<String>,
) -> std::io::Result<()> {
    write_msg(w, &RelayMsg::error(code, message)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples() -> Vec<RelayMsg> {
        vec![
            RelayMsg::reserve(300, "peer-b"),
            RelayMsg::reserved(7),
            RelayMsg::connect(7),
            RelayMsg::bound(7),
            RelayMsg::punch_req("peer-b", vec!["1.2.3.4:5".into()]),
            RelayMsg::punch_ack("peer-a", vec!["5.6.7.8:9".into(), "[::1]:1".into()]),
            RelayMsg::error(errcode::UNKNOWN_CIRCUIT, "no such circuit"),
        ]
    }

    #[tokio::test]
    async fn roundtrip_all_kinds_over_duplex() {
        for msg in samples() {
            let (mut tx, mut rx) = tokio::io::duplex(256);
            write_msg(&mut tx, &msg).await.unwrap();
            let got = read_msg(&mut rx).await.unwrap().expect("one frame");
            assert_eq!(got, msg);
        }
    }

    #[tokio::test]
    async fn clean_close_reads_none() {
        let (tx, mut rx) = tokio::io::duplex(64);
        drop(tx);
        assert!(read_msg(&mut rx).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn oversized_length_prefix_rejected() {
        let (mut tx, mut rx) = tokio::io::duplex(64);
        let mut buf = Vec::new();
        push_varint(&mut buf, (MAX_FRAME + 1) as u64);
        tx.write_all(&buf).await.unwrap();
        let err = read_msg(&mut rx).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }
}
