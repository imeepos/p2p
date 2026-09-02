//! echo 协议接入与 peer_dial target 解析（gui-contract.md §6）。
//!
//! 协议常量与 crates/p2p-cli/src/echo.rs 同值：契约 §4 限定 path 依赖仅 crates/p2p
//! （含传递依赖），不引入 p2p-cli，故本地复刻常量并注明来源，两处必须保持一致。

use std::io;
use std::net::IpAddr;

use p2p::{BoxedStream, PeerId, ProtocolHandler, ProtocolId};
use p2p_protocol::{read_frame, write_frame};

/// /p2p-lab/echo/1 回声协议 ID（同 p2p-cli::echo::ECHO_PROTOCOL）。
pub const ECHO_PROTOCOL: &str = "/p2p-lab/echo/1";

/// ping 载荷（同 p2p-cli::echo::PING_PAYLOAD）。
pub const PING_PAYLOAD: &[u8] = b"p2p-ping";

/// 回声 handler：收一帧原样回一帧（同 p2p-cli::echo::EchoHandler）。
pub struct EchoHandler;

#[async_trait::async_trait]
impl ProtocolHandler for EchoHandler {
    fn protocol(&self) -> ProtocolId {
        ProtocolId::new(ECHO_PROTOCOL).expect("内置 echo 协议 id 合法")
    }

    async fn handle(&self, mut stream: BoxedStream) -> io::Result<()> {
        let frame = read_frame(&mut stream).await?;
        write_frame(&mut stream, &frame).await
    }
}

/// 解析 base58 PeerId（32 字节定长；语义同 p2p-cli::cli::parse_peer_id）。
pub fn parse_peer_id(s: &str) -> Result<PeerId, String> {
    let bytes = bs58::decode(s)
        .into_vec()
        .map_err(|e| format!("PeerId 不是合法 base58: {e}"))?;
    let len = bytes.len();
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("PeerId 必须恰好 32 字节，实际 {len}"))?;
    Ok(PeerId::from_bytes(arr))
}

/// 解析 "<peer_id>@<addr>"（契约 §6）；addr 为 ip/u端口（QUIC）或 ip/t端口（TCP）。
pub fn parse_target(target: &str) -> Result<(PeerId, String), String> {
    let (peer_str, addr) = target.split_once('@').ok_or_else(|| {
        format!("target 缺少 '@' 分隔符，应为 <peer_id>@<addr>，实得 \"{target}\"")
    })?;
    let peer = parse_peer_id(peer_str)?;
    validate_addr(addr)?;
    Ok((peer, addr.to_string()))
}

/// 校验 addr 语法（与 p2p rendezvous 的 parse_transport_addr 同规则）。
fn validate_addr(addr: &str) -> Result<(), String> {
    let bad = || format!("非法地址 \"{addr}\"，应为 ip/u端口（QUIC）或 ip/t端口（TCP）");
    let (ip_str, tail) = addr.split_once('/').ok_or_else(bad)?;
    ip_str.parse::<IpAddr>().map_err(|_| bad())?;
    let mut chars = tail.chars();
    let kind = chars.next().ok_or_else(bad)?;
    if kind != 'u' && kind != 't' {
        return Err(bad());
    }
    let port: u16 = chars.as_str().parse().map_err(|_| bad())?;
    if port == 0 {
        return Err(format!("端口不能为 0：\"{addr}\""));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_peer() -> String {
        bs58::encode([7u8; 32]).into_string()
    }

    #[test]
    fn parse_peer_id_roundtrips_base58() {
        let s = sample_peer();
        let peer = parse_peer_id(&s).expect("合法 PeerId");
        assert_eq!(peer.to_string(), s);
    }

    #[test]
    fn parse_peer_id_rejects_bad_base58_and_length() {
        assert!(parse_peer_id("!!!").is_err());
        let short = bs58::encode([7u8; 31]).into_string();
        assert!(parse_peer_id(&short).is_err());
    }

    #[test]
    fn parse_target_accepts_quic_and_tcp_addrs() {
        let target = format!("{}@192.168.1.5/3400", sample_peer());
        let (peer, addr) = parse_target(&target).expect("合法 target");
        assert_eq!(peer.to_string(), sample_peer());
        assert_eq!(addr, "192.168.1.5/3400");

        let target = format!("{}@[::1]/t3401", sample_peer());
        let (_, addr) = parse_target(&target).expect("合法 tcp target");
        assert_eq!(addr, "[::1]/t3401");
    }

    #[test]
    fn parse_target_rejects_missing_separator() {
        assert!(parse_target("no-separator").is_err());
    }

    #[test]
    fn parse_target_rejects_bad_peer_or_addr() {
        assert!(parse_target("zzz-not-base58@1.2.3.4/3400").is_err());
        let bad_addr = format!("{}@1.2.3.4", sample_peer());
        assert!(parse_target(&bad_addr).is_err());
        let bad_kind = format!("{}@1.2.3.4/x3400", sample_peer());
        assert!(parse_target(&bad_kind).is_err());
        let bad_port = format!("{}@1.2.3.4/u99999", sample_peer());
        assert!(parse_target(&bad_port).is_err());
        let zero_port = format!("{}@1.2.3.4/u0", sample_peer());
        assert!(parse_target(&zero_port).is_err());
    }
}
