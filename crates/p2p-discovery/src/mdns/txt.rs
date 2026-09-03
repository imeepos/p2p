//! mDNS TXT 记录编解码：peer 身份与端口随通告播散，对端据此还原地址集。

use std::net::IpAddr;

use mdns_sd::ServiceInfo;
use p2p_identity::PeerId;
use p2p_transport::TransportAddr;

const TXT_KEY_PEER: &str = "peer";
const TXT_KEY_QUIC: &str = "quic";
const TXT_KEY_TCP: &str = "tcp";

/// 编码通告 TXT 记录：peer(base58) + 可选 quic/tcp 端口。
pub(super) fn encode_txt(
    peer: &PeerId,
    quic: Option<u16>,
    tcp: Option<u16>,
) -> Vec<(&'static str, String)> {
    let mut props = vec![(TXT_KEY_PEER, peer.to_string())];
    if let Some(port) = quic {
        props.push((TXT_KEY_QUIC, port.to_string()));
    }
    if let Some(port) = tcp {
        props.push((TXT_KEY_TCP, port.to_string()));
    }
    props
}

/// 从已解析的 ServiceInfo 解码 (PeerId, 地址列表)。peer 缺失或端口非法返回 None。
/// 地址集全量展开（addr_auto 通告携带本机全部接口地址）：只取首地址会丢掉多接口
/// 主机的其余合法候选，配合地址簿只增不汰会放大死地址拨号风暴（2026-09-04）。
/// 逐地址的 loopback/link-local 过滤与去重统一由地址簿入簿卫生把关。
pub(super) fn decode_txt(info: &ServiceInfo) -> Option<(PeerId, Vec<TransportAddr>)> {
    let peer_b58 = info.get_property_val_str(TXT_KEY_PEER)?;
    let bytes: [u8; 32] = bs58::decode(peer_b58).into_vec().ok()?.try_into().ok()?;
    let peer = PeerId::from_bytes(bytes);
    let quic = txt_port(info, TXT_KEY_QUIC);
    let tcp = txt_port(info, TXT_KEY_TCP);
    let mut addrs = Vec::new();
    for ip in info.get_addresses() {
        if let Some(port) = quic {
            addrs.push(TransportAddr::Quic { ip: *ip, port });
        }
        if let Some(port) = tcp {
            addrs.push(TransportAddr::Tcp { ip: *ip, port });
        }
    }
    (!addrs.is_empty()).then_some((peer, addrs))
}

/// 读取 TXT 端口属性并解析为 u16。
fn txt_port(info: &ServiceInfo, key: &str) -> Option<u16> {
    info.get_property_val_str(key).and_then(|s| s.parse().ok())
}
