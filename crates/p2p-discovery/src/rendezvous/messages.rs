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
        let port = self.port as u16;
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

/// 签名字段：namespace + peer_id + addrs 的 protobuf 序列化字节。
#[derive(Clone, PartialEq, prost::Message)]
struct SignedFields {
    #[prost(string, tag = "1")]
    namespace: String,
    #[prost(bytes, tag = "2")]
    peer_id: Vec<u8>,
    #[prost(message, repeated, tag = "3")]
    addrs: Vec<AddrMsg>,
}

pub fn signed_payload(namespace: &str, peer_id: &PeerId, addrs: &[TransportAddr]) -> Vec<u8> {
    SignedFields {
        namespace: namespace.to_string(),
        peer_id: peer_id.as_bytes().to_vec(),
        addrs: addrs.iter().map(AddrMsg::from_addr).collect(),
    }
    .encode_to_vec()
}

/// 用身份私钥对 (namespace, peer_id, addrs) 签名，构造注册消息。
pub fn sign_register(
    kp: &Keypair,
    namespace: &str,
    addrs: &[TransportAddr],
    ttl_secs: u32,
) -> Register {
    let peer_id = kp.peer_id();
    let sig = kp.sign(&signed_payload(namespace, &peer_id, addrs));
    Register {
        namespace: namespace.to_string(),
        peer_id: peer_id.as_bytes().to_vec(),
        pubkey: kp.public().to_vec(),
        addrs: addrs.iter().map(AddrMsg::from_addr).collect(),
        ttl_secs,
        sig: sig.to_vec(),
    }
}

/// 校验注册：peer_id 必须与公钥自证一致，且 (namespace, peer_id, addrs) 签名有效。
pub fn verify_register(reg: &Register) -> bool {
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
    let addrs: Vec<TransportAddr> = reg.addrs.iter().filter_map(AddrMsg::to_addr).collect();
    Keypair::verify(
        &pubkey,
        &signed_payload(&reg.namespace, &expected, &addrs),
        &sig,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_addrs() -> Vec<TransportAddr> {
        vec![
            TransportAddr::Quic {
                ip: "10.0.0.1".parse().unwrap(),
                port: 4000,
            },
            TransportAddr::Tcp {
                ip: "10.0.0.1".parse().unwrap(),
                port: 4001,
            },
        ]
    }

    #[test]
    fn signed_register_passes_verification() {
        let kp = Keypair::generate();
        let reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
        assert!(verify_register(&reg));
    }

    #[test]
    fn tampered_addrs_rejected() {
        let kp = Keypair::generate();
        let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
        reg.addrs[0].port = 9999;
        assert!(!verify_register(&reg));
    }

    #[test]
    fn tampered_namespace_rejected() {
        let kp = Keypair::generate();
        let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
        reg.namespace = "room-b".to_string();
        assert!(!verify_register(&reg));
    }

    #[test]
    fn wrong_peer_id_rejected() {
        let kp = Keypair::generate();
        let other = Keypair::generate();
        let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
        reg.peer_id = other.peer_id().as_bytes().to_vec();
        assert!(!verify_register(&reg));
    }

    #[test]
    fn wrong_signature_rejected() {
        let kp = Keypair::generate();
        let attacker = Keypair::generate();
        let mut reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
        reg.sig = attacker
            .sign(&signed_payload("room-a", &kp.peer_id(), &sample_addrs()))
            .to_vec();
        assert!(!verify_register(&reg));
    }

    #[test]
    fn prost_roundtrip_register_query_response() {
        let kp = Keypair::generate();
        let reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
        let decoded = Register::decode(reg.encode_to_vec().as_slice()).expect("decode");
        assert_eq!(reg, decoded);

        let query = Query {
            namespace: "room-a".into(),
            peer_id: reg.peer_id.clone(),
        };
        let q2 = Query::decode(query.encode_to_vec().as_slice()).expect("decode");
        assert_eq!(query, q2);

        let resp = Response {
            error: String::new(),
            peers: vec![PeerEntry {
                peer_id: reg.peer_id.clone(),
                addrs: reg.addrs.clone(),
            }],
        };
        let r2 = Response::decode(resp.encode_to_vec().as_slice()).expect("decode");
        assert_eq!(resp, r2);
    }

    #[test]
    fn request_oneof_roundtrip() {
        let kp = Keypair::generate();
        let reg = sign_register(&kp, "room-a", &sample_addrs(), 60);
        let req = Request::register(reg);
        let bytes = req.encode_to_vec();
        let decoded = Request::decode(bytes.as_slice()).expect("decode");
        assert_eq!(req, decoded);
        assert!(matches!(decoded.kind, Some(request::Kind::Register(_))));
    }

    #[test]
    fn addr_conversion_roundtrip() {
        for addr in sample_addrs() {
            assert_eq!(AddrMsg::from_addr(&addr).to_addr(), Some(addr));
        }
    }
}
