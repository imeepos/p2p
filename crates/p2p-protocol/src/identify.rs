//! identify 控制面消息（prost 手写 derive，风格对齐 relay/rendezvous messages）。
//!
//! v1 语义（specs/identify.md）：Request→Response 一问一答，一流一事务；
//! protobuf 字段只增不改（tag 与类型冻结）。地址形态与 rendezvous AddrMsg
//! 同构（quic bool / ip string / port u32），独立定义避免 p2p-protocol 反向
//! 依赖 p2p-discovery；TransportAddr 转换助手归 p2p-swarm（依赖 p2p-transport）。

/// 请求协议版本；服务端只接受等于本值的请求，其余关流拒绝。
pub const PROTOCOL_VERSION: u32 = 1;

/// 请求：一问一答，单流单事务，单帧单消息。
#[derive(Clone, PartialEq, prost::Message)]
pub struct Request {
    /// 必须等于 [PROTOCOL_VERSION]；不匹配由服务端关流拒绝（显式失败）。
    #[prost(uint32, tag = "1")]
    pub protocol_version: u32,
}

/// 地址在线上的表示：quic 标记 + ip 字符串 + 端口（rendezvous AddrMsg 同构）。
#[derive(Clone, PartialEq, prost::Message)]
pub struct AddrMsg {
    #[prost(bool, tag = "1")]
    pub quic: bool,
    #[prost(string, tag = "2")]
    pub ip: String,
    #[prost(uint32, tag = "3")]
    pub port: u32,
}

/// 响应：全部字段是应答方自报线索，身份认定只来自握手（specs/identify.md §5）。
#[derive(Clone, PartialEq, prost::Message)]
pub struct Response {
    /// 应答方 ed25519 公钥原始 32 字节；收端必须与握手推导 PeerId 交叉核对，
    /// 不一致按协议违规断流，不得采信。
    #[prost(bytes, tag = "1")]
    pub pubkey: Vec<u8>,
    /// 应答方监听地址；空列表合法（纯客户端节点可无监听）。
    #[prost(message, repeated, tag = "2")]
    pub listen_addrs: Vec<AddrMsg>,
    /// 服务端在本连接上观测到的请求端远端地址；无法观测（中继电路、裸流）时缺省。
    #[prost(message, optional, tag = "3")]
    pub observed_addr: Option<AddrMsg>,
    /// 形如 "p2p-base/0.1.0"；缺省合法。
    #[prost(string, optional, tag = "4")]
    pub software: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    #[test]
    fn response_roundtrip_preserves_all_fields() {
        let resp = Response {
            pubkey: vec![7u8; 32],
            listen_addrs: vec![AddrMsg {
                quic: true,
                ip: "127.0.0.1".into(),
                port: 3400,
            }],
            observed_addr: Some(AddrMsg {
                quic: false,
                ip: "203.0.113.9".into(),
                port: 51000,
            }),
            software: Some("p2p-base/0.1.0".into()),
        };
        let bytes = resp.encode_to_vec();
        let back = Response::decode(bytes.as_slice()).expect("decode");
        assert_eq!(resp, back);
    }

    #[test]
    fn optional_fields_default_to_none_and_version_is_one() {
        let resp =
            Response::decode(Response::default().encode_to_vec().as_slice()).expect("decode");
        assert!(resp.pubkey.is_empty());
        assert!(resp.listen_addrs.is_empty());
        assert!(resp.observed_addr.is_none());
        assert!(resp.software.is_none());
        assert_eq!(PROTOCOL_VERSION, 1);
    }
}
