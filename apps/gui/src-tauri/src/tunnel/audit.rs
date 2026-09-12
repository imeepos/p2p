//! tunnel 访侧装配残留面：入口链接拼装（§6 open_url）。
//! 会话审计（原 ConnAudit/AuditLog）已下沉 p2p-tunnel local_proxy（W-TB：
//! SessionAudit/SessionLog 活账 + TunnelAuditRecord 快照）；其生命周期单测
//! 随迁 crate（local_proxy::session::tests）；camelCase 映射在 types.rs。

use std::net::SocketAddr;

/// 反代入口链接拼装（§6：open_url = http://<local_addr>/?token=<同 token>）。
/// token 空串 = 通用形态「无 token」语义（§19.3-9）：不拼装 `?token=`，
/// open_url 即 local_addr 本身；DSH 形态 token 恒非空（parse_dsh_url 保证）。
pub fn open_url(local_addr: SocketAddr, token: &str) -> String {
    if token.is_empty() {
        return format!("http://{local_addr}");
    }
    format!("http://{local_addr}/?token={token}")
}
