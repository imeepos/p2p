//! 隧道数据泵导出面（契约 §1/§4 分块与半关闭 / §3.3 流式转发）：签名桩。
//! 请求体精确转发（bytesOut）与响应逐块排干（bytesIn）；流式转发禁整包缓冲；
//! 读侧容忍任意 ≤1 MiB 分块边界。实现 = W-TB（迁移自
//! apps/gui/src-tauri/src/tunnel/pump.rs；字节计数缝由 GUI ConnAudit 换为
//! [PumpAudit] trait，crate 内零 GUI 依赖）。

use std::time::Duration;

/// 隧道 IO 空闲护栏：响应/长连接期间读停滞超过该值即放弃（禁无限静默悬挂）。
/// 出处：apps/gui/src-tauri/src/tunnel/pump.rs:10-11。
pub const TUNNEL_IDLE_GRACE: Duration = Duration::from_secs(30);

/// 泵侧字节计数缝（§5.2 审计 bytes_in/bytes_out，记录方视角）。
/// 提炼自 apps/gui/src-tauri/src/tunnel/pump.rs 对 ConnAudit 的 add_in/add_out
/// 消费面（gui tunnel/audit.rs）；W-TB 迁移时由 ConnAudit 实现本 trait 或以
/// crate 内会话审计结构承载。
pub trait PumpAudit: Send + Sync {
    /// 自隧道收到 n 字节（响应方向）。
    fn add_in(&self, n: u64);
    /// 向隧道发出 n 字节（请求方向）。
    fn add_out(&self, n: u64);
}

/// 读应答头原始字节（带 [TUNNEL_IDLE_GRACE] 护栏）+ 头后已到的 body 字节。
/// 超时/IO 错误为人话 Err。出处：apps/gui/src-tauri/src/tunnel/pump.rs:13-21。
pub async fn read_reply_head(
    stream: &mut (impl tokio::io::AsyncRead + Unpin),
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let _ = stream;
    todo!("W-TB")
}

/// 应答首行是否 101 升级协议切换（HTTP/1.0 或 HTTP/1.1）。
/// 出处：apps/gui/src-tauri/src/tunnel/pump.rs:23-27。
pub fn is_101(raw: &[u8]) -> bool {
    let _ = raw;
    todo!("W-TB")
}

/// 请求体精确转发：leftover 先落，再补足 Content-Length 差额；逐块计
/// bytesOut；body 在 Content-Length 之前 EOF 即 Err。
/// 出处：apps/gui/src-tauri/src/tunnel/pump.rs:29-61。
pub async fn forward_exact(
    src: &mut (impl tokio::io::AsyncRead + Unpin + Send),
    dst: &mut (impl tokio::io::AsyncWrite + Unpin + Send),
    leftover: Vec<u8>,
    content_len: u64,
    audit: &dyn PumpAudit,
) -> Result<(), String> {
    let _ = (src, dst, leftover, content_len, audit);
    todo!("W-TB")
}

/// 隧道 → 本地逐块排干（流式，禁整包缓冲）；EOF 即响应终点并关本地写半；
/// 逐块计 bytesIn；读停滞超 [TUNNEL_IDLE_GRACE] 即 Err。
/// 出处：apps/gui/src-tauri/src/tunnel/pump.rs:63-94。
pub async fn drain_reply(
    tunnel: &mut (impl tokio::io::AsyncRead + Unpin),
    local: &mut (impl tokio::io::AsyncWrite + Unpin),
    first: Vec<u8>,
    audit: &dyn PumpAudit,
) -> Result<(), String> {
    let _ = (tunnel, local, first, audit);
    todo!("W-TB")
}
