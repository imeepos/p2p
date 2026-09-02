//! echo 协议接入与 peer_dial target 解析（gui-contract.md §6）。
//!
//! 协议常量与 handler 复用 p2p-cli 公开导出（pub mod echo，协调者裁决允许
//! path 依赖 p2p-cli），与 CLI ping 走同一份协议实现，不重定义。

use std::net::IpAddr;

use p2p::PeerId;

pub use p2p_cli::echo::{EchoHandler, ECHO_PROTOCOL, PING_PAYLOAD};

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
        // u/t 前缀与 TransportAddr 展示格式一致（契约 §6）
        let target = format!("{}@192.168.1.5/u3400", sample_peer());
        let (peer, addr) = parse_target(&target).expect("合法 target");
        assert_eq!(peer.to_string(), sample_peer());
        assert_eq!(addr, "192.168.1.5/u3400");

        // 裸 IPv6（无方括号）与内核 parse_transport_addr 行为一致
        let target = format!("{}@::1/t3401", sample_peer());
        let (_, addr) = parse_target(&target).expect("合法 tcp target");
        assert_eq!(addr, "::1/t3401");
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
        // 缺 u/t 前缀与内核 parse_transport_addr 同判非法
        let bare_quic = format!("{}@1.2.3.4/3400", sample_peer());
        assert!(parse_target(&bare_quic).is_err());
        let bad_kind = format!("{}@1.2.3.4/x3400", sample_peer());
        assert!(parse_target(&bad_kind).is_err());
        let bad_port = format!("{}@1.2.3.4/u99999", sample_peer());
        assert!(parse_target(&bad_port).is_err());
        let zero_port = format!("{}@1.2.3.4/u0", sample_peer());
        assert!(parse_target(&zero_port).is_err());
    }
}
