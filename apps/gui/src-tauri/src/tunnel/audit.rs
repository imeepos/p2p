//! tunnel 访侧装配残留面：入口链接拼装（§6 open_url）。
//! 会话审计（原 ConnAudit/AuditLog）已下沉 p2p-tunnel local_proxy（W-TB：
//! SessionAudit/SessionLog 活账 + TunnelAuditRecord 快照）；其生命周期单测
//! 随迁 crate（local_proxy::session::tests）；camelCase 映射在 types.rs。

use std::net::SocketAddr;

/// 反代入口链接拼装（§6：open_url = http://<local_addr>/?token=<同 token>）。
pub fn open_url(local_addr: SocketAddr, token: &str) -> String {
    format!("http://{local_addr}/?token={token}")
}
