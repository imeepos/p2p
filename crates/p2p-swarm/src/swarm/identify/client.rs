//! identify 发起端（客户端）：`Swarm::identify()` 一问一答 + 身份交叉核对。

use p2p_identity::PeerId;
use p2p_protocol::identify::{AddrMsg, Request, Response, PROTOCOL_VERSION};
use p2p_protocol::{open_with_protocol, read_frame, write_frame, ProtocolError};
use p2p_transport::TransportAddr;
use prost::Message;
use tokio::time::timeout;

use super::{identify_id, invalid, IDENTIFY_TIMEOUT};
use crate::Swarm;

/// identify 客户端结果。listen_addrs 已解析为可拨地址；不可解析条目已丢弃。
#[derive(Clone, Debug)]
pub struct IdentifyInfo {
    pub peer: PeerId,
    pub pubkey: [u8; 32],
    pub listen_addrs: Vec<TransportAddr>,
    pub observed_addr: Option<TransportAddr>,
    pub software: Option<String>,
}

impl Swarm {
    /// identify 客户端：开流 → 一问一答 → pubkey 与握手 PeerId 交叉核对。
    /// 不一致按协议违规断流（错误文本带 mismatch）；对端版本不兼容关流时
    /// 以 EOF 类 io 错误收敛。整体受事务超时约束；本实现不缓存响应
    /// （可缓存 TTL ≤60s，由调用方决定）。
    pub async fn identify(&self, peer: PeerId) -> Result<IdentifyInfo, ProtocolError> {
        match timeout(IDENTIFY_TIMEOUT, self.identify_once(peer)).await {
            Ok(result) => result,
            Err(_) => Err(ProtocolError::Timeout(IDENTIFY_TIMEOUT)),
        }
    }

    async fn identify_once(&self, peer: PeerId) -> Result<IdentifyInfo, ProtocolError> {
        let id = identify_id();
        let raw = self.open_stream(&peer, &id).await?;
        let mut stream = open_with_protocol(raw, &id).await?;
        let req = Request {
            protocol_version: PROTOCOL_VERSION,
        };
        write_frame(&mut stream, &req.encode_to_vec()).await?;
        let resp = read_frame(&mut stream).await?;
        let resp = Response::decode(resp.as_slice())
            .map_err(|e| ProtocolError::Io(invalid(format!("identify response decode: {e}"))))?;
        Self::verify_response(peer, &resp)
    }

    /// 交叉核对：握手推导 PeerId 为准，pubkey 推导不一致即协议违规断流。
    fn verify_response(peer: PeerId, resp: &Response) -> Result<IdentifyInfo, ProtocolError> {
        let pubkey: [u8; 32] = resp.pubkey.as_slice().try_into().map_err(|_| {
            ProtocolError::Io(invalid(format!(
                "identify pubkey must be 32 bytes, got {}",
                resp.pubkey.len()
            )))
        })?;
        let derived = PeerId::from_public_key(&pubkey);
        if derived != peer {
            tracing::warn!(%peer, derived = %derived, "identify pubkey mismatch, dropping stream");
            return Err(ProtocolError::Io(invalid(format!(
                "identify pubkey mismatch: response derives {derived}, handshake peer is {peer}"
            ))));
        }
        let listen_addrs = parse_addrs(&resp.listen_addrs);
        let observed_addr = resp.observed_addr.as_ref().and_then(|m| {
            let parsed = addr_of(m);
            if parsed.is_none() {
                tracing::debug!(
                    quic = m.quic,
                    ip = %m.ip,
                    port = m.port,
                    "identify observed addr unparseable"
                );
            }
            parsed
        });
        Ok(IdentifyInfo {
            peer,
            pubkey,
            listen_addrs,
            observed_addr,
            software: resp.software.clone(),
        })
    }
}

/// 解析对端自报地址；不可解析条目按线索丢弃并留 debug（identify 管地址线索，
/// 与 rendezvous 注册的整单拒绝不同——此处不做准入，只做展示/拨号候选）。
fn addr_of(msg: &AddrMsg) -> Option<TransportAddr> {
    let ip: std::net::IpAddr = msg.ip.parse().ok()?;
    let port = u16::try_from(msg.port).ok()?;
    Some(if msg.quic {
        TransportAddr::Quic { ip, port }
    } else {
        TransportAddr::Tcp { ip, port }
    })
}

fn parse_addrs(msgs: &[AddrMsg]) -> Vec<TransportAddr> {
    msgs.iter()
        .filter_map(|m| match addr_of(m) {
            Some(a) => Some(a),
            None => {
                tracing::debug!(
                    quic = m.quic,
                    ip = %m.ip,
                    port = m.port,
                    "identify listen addr unparseable"
                );
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Swarm;
    use p2p_identity::Keypair;
    use std::time::Duration;

    #[test]
    fn verify_response_cross_checks_pubkey_against_handshake_peer() {
        let other = Keypair::from_seed(&[1u8; 32]).peer_id();
        let short = Response {
            pubkey: vec![0u8; 8],
            ..Response::default()
        };
        let err = Swarm::verify_response(other, &short).expect_err("bad length must fail");
        assert!(err.to_string().contains("32 bytes"), "{err}");
        let impostor = Keypair::from_seed(&[2u8; 32]);
        let graft = Response {
            pubkey: impostor.public().to_vec(),
            ..Response::default()
        };
        let err = Swarm::verify_response(other, &graft).expect_err("mismatch must fail");
        assert!(err.to_string().contains("mismatch"), "{err}");
    }

    #[test]
    fn addr_msg_roundtrip_and_unparseable_dropped() {
        let addr = TransportAddr::Quic {
            ip: "127.0.0.1".parse().expect("ip"),
            port: 3400,
        };
        let msg = AddrMsg {
            quic: true,
            ip: "127.0.0.1".into(),
            port: 3400,
        };
        assert_eq!(addr_of(&msg), Some(addr));
        let bad = AddrMsg {
            quic: false,
            ip: "nope".into(),
            port: 70000,
        };
        assert_eq!(addr_of(&bad), None);
    }

    #[test]
    fn timeout_is_documented_value() {
        assert_eq!(IDENTIFY_TIMEOUT, Duration::from_secs(10));
    }
}
