//! rendezvous 控制面消息（prost 手写 derive，不引入 protoc）与注册签名/验证。

use std::net::IpAddr;

use p2p_identity::{Keypair, PeerId};
use p2p_transport::TransportAddr;
use prost::Message;

/// 地址在 wire 上的表示：quic 标记 + ip 字符串 + 端口。
#[derive(Clone, PartialEq, prost::Message)]
pub struct AddrMsg {
    #[prost(bool, tag = "1")]
    pub quic: bool,
    #[prost(string, tag = "2")]
    pub ip: String,
    #[prost(uint32, tag = "3")]
    pub port: u32,
}

impl AddrMsg {
    pub fn from_addr(addr: &TransportAddr) -> Self {
        match addr {
            TransportAddr::Quic { ip, port } => Self {
                quic: true,
                ip: ip.to_string(),
                port: *port as u32,
            },
            TransportAddr::Tcp { ip, port } => Self {
                quic: false,
                ip: ip.to_string(),
                port: *port as u32,
            },
        }
    }

    pub fn to_addr(&self) -> Option<TransportAddr> {
        let ip: IpAddr = self.ip.parse().ok()?;
        // L2：port 超 u16 显式拒绝，不做静默截断
        let port = u16::try_from(self.port).ok()?;
        if self.quic {
            Some(TransportAddr::Quic { ip, port })
        } else {
            Some(TransportAddr::Tcp { ip, port })
        }
    }
}

/// 注册：携带公钥与签名，服务端据 pubkey 校验 peer_id 绑定与签名。
#[derive(Clone, PartialEq, prost::Message)]
pub struct Register {
    #[prost(string, tag = "1")]
    pub namespace: String,
    #[prost(bytes, tag = "2")]
    pub peer_id: Vec<u8>,
    #[prost(bytes, tag = "3")]
    pub pubkey: Vec<u8>,
    #[prost(message, repeated, tag = "4")]
    pub addrs: Vec<AddrMsg>,
    #[prost(uint32, tag = "5")]
    pub ttl_secs: u32,
    #[prost(bytes, tag = "6")]
    pub sig: Vec<u8>,
    /// 注册时刻（unix 秒），签名覆盖，服务端据此做重放窗口校验。
    #[prost(uint64, tag = "7")]
    pub issued_at: u64,
}

/// 按 PeerId 查询；peer_id 为空表示查询整个 namespace。
#[derive(Clone, PartialEq, prost::Message)]
pub struct Query {
    #[prost(string, tag = "1")]
    pub namespace: String,
    #[prost(bytes, tag = "2")]
    pub peer_id: Vec<u8>,
}

/// 单个对端条目。
#[derive(Clone, PartialEq, prost::Message)]
pub struct PeerEntry {
    #[prost(bytes, tag = "1")]
    pub peer_id: Vec<u8>,
    #[prost(message, repeated, tag = "2")]
    pub addrs: Vec<AddrMsg>,
}

/// 应答：error 非空表示失败（如注册签名校验不通过），peers 为查询结果。
#[derive(Clone, PartialEq, prost::Message)]
pub struct Response {
    #[prost(string, tag = "1")]
    pub error: String,
    #[prost(message, repeated, tag = "2")]
    pub peers: Vec<PeerEntry>,
}

/// 客户端上行消息：注册或查询。
#[derive(Clone, PartialEq, prost::Message)]
pub struct Request {
    #[prost(oneof = "request::Kind", tags = "1, 2")]
    pub kind: Option<request::Kind>,
}

pub mod request {
    #[derive(Clone, PartialEq, prost::Oneof)]
    pub enum Kind {
        #[prost(message, tag = "1")]
        Register(super::Register),
        #[prost(message, tag = "2")]
        Query(super::Query),
    }
}

impl Request {
    pub fn register(reg: Register) -> Self {
        Self {
            kind: Some(request::Kind::Register(reg)),
        }
    }

    pub fn query(namespace: String, peer_id: Vec<u8>) -> Self {
        Self {
            kind: Some(request::Kind::Query(Query { namespace, peer_id })),
        }
    }
}

impl Response {
    pub fn ok() -> Self {
        Self {
            error: String::new(),
            peers: Vec::new(),
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            error: msg.into(),
            peers: Vec::new(),
        }
    }

    /// 失败即返回错误信息（服务端/客户端两侧统一的失败信号）。
    pub fn ensure_ok(&self) -> Result<(), String> {
        if self.error.is_empty() {
            Ok(())
        } else {
            Err(self.error.clone())
        }
    }
}

/// 签名新鲜度容差（秒）：注册时刻距本机时钟超此即拒，防旧帧重放（H1）。
pub const FRESH_TOLERANCE_SECS: u64 = 300;

/// 当前 unix 秒（客户端签发、服务端校验新鲜度的统一时钟源）。
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 签名字段：namespace/peer_id/addrs/ttl_secs/issued_at 的 protobuf 序列化字节。
/// ttl 与 issued_at 均入签名，杜绝"篡改 TTL 或重放旧帧仍验签通过"（H1）。
#[derive(Clone, PartialEq, prost::Message)]
struct SignedFields {
    #[prost(string, tag = "1")]
    namespace: String,
    #[prost(bytes, tag = "2")]
    peer_id: Vec<u8>,
    #[prost(message, repeated, tag = "3")]
    addrs: Vec<AddrMsg>,
    #[prost(uint32, tag = "4")]
    ttl_secs: u32,
    #[prost(uint64, tag = "5")]
    issued_at: u64,
}

pub fn signed_payload(
    namespace: &str,
    peer_id: &PeerId,
    addrs: &[TransportAddr],
    ttl_secs: u32,
    issued_at: u64,
) -> Vec<u8> {
    SignedFields {
        namespace: namespace.to_string(),
        peer_id: peer_id.as_bytes().to_vec(),
        addrs: addrs.iter().map(AddrMsg::from_addr).collect(),
        ttl_secs,
        issued_at,
    }
    .encode_to_vec()
}

/// 用身份私钥对 (namespace, peer_id, addrs, ttl, issued_at) 签名，构造注册消息。
pub fn sign_register(
    kp: &Keypair,
    namespace: &str,
    addrs: &[TransportAddr],
    ttl_secs: u32,
    issued_at: u64,
) -> Register {
    let peer_id = kp.peer_id();
    let sig = kp.sign(&signed_payload(
        namespace, &peer_id, addrs, ttl_secs, issued_at,
    ));
    Register {
        namespace: namespace.to_string(),
        peer_id: peer_id.as_bytes().to_vec(),
        pubkey: kp.public().to_vec(),
        addrs: addrs.iter().map(AddrMsg::from_addr).collect(),
        ttl_secs,
        issued_at,
        sig: sig.to_vec(),
    }
}

/// 纯密码学校验：peer_id 与公钥绑定、签名覆盖全字段、所有地址可解析（L2 整单拒绝）。
fn verify_signature(reg: &Register) -> bool {
    let pubkey: [u8; 32] = match reg.pubkey.as_slice().try_into() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let expected = PeerId::from_public_key(&pubkey);
    let peer_id: [u8; 32] = match reg.peer_id.as_slice().try_into() {
        Ok(p) => p,
        Err(_) => return false,
    };
    if PeerId::from_bytes(peer_id) != expected {
        return false;
    }
    let sig: [u8; 64] = match reg.sig.as_slice().try_into() {
        Ok(s) => s,
        Err(_) => return false,
    };
    // L2：任一地址解析失败（含端口越界）即拒绝整个注册，不再 filter_map 静默丢弃
    // 空地址列表允许（查询型节点可无监听地址，仅作在场标记）
    let addrs: Option<Vec<TransportAddr>> = reg.addrs.iter().map(AddrMsg::to_addr).collect();
    let Some(addrs) = addrs else {
        return false;
    };
    Keypair::verify(
        &pubkey,
        &signed_payload(
            &reg.namespace,
            &expected,
            &addrs,
            reg.ttl_secs,
            reg.issued_at,
        ),
        &sig,
    )
}

/// 校验注册：密码学 + 时间新鲜度（now 为服务端当前 unix 秒）。
/// 时间偏差超过 [FRESH_TOLERANCE_SECS] 即拒，构成重放窗口（H1）。
pub fn verify_register(reg: &Register, now: u64) -> bool {
    verify_signature(reg) && now.abs_diff(reg.issued_at) <= FRESH_TOLERANCE_SECS
}

#[cfg(test)]
mod tests;
