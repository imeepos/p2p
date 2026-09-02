//! relay 控制面消息（design 5.3/5.4）：prost derive 手写，免 protoc。
//!
//! 线格式（varint 长度前缀 + RelayMsg protobuf 帧）见 frame 模块。
//! Punch 信令语义：外发消息的 peer_id 指明目的地；relay 转发时改写为
//! 发送方，接收方看到的 peer_id 即对端真实身份。

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
