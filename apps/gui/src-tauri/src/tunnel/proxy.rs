//! 本地回环反代（gui-contract §19.3 / 冻结契约 §5-§6）：只绑 `127.0.0.1`
//! 字面量，端口 0 由 OS 分配；一条浏览器连接 ↔ 一条隧道流（acp-pump
//! 「一连接一泵」体例）。每请求重写 Host 为目标 `127.0.0.1:<port>`；非升级
//! 请求 Connection: close（hop-by-hop 取舍：响应终点=隧道 EOF，免复用语义）；
//! WebSocket 升级 101 后与隧道流裸字节双向泵；响应与 body 逐块转发禁整包
//! 缓冲。每条隧道流一条审计（audit::ConnAudit，uid 同源两侧日志）。

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use p2p::BoxedStream;
use tokio::io::{AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::audit::ConnAudit;
use super::client::{open_tunnel, TunnelDialer, FIRST_REPLY_TIMEOUT};
use super::head::{read_head, Head};
use super::pump::{drain_reply, forward_exact, is_101, read_reply_head};
use super::ticket::TunnelTicket;

/// 反代共享上下文。
pub struct ProxyCtx {
    target_port: u16,
    peer_id: String,
    dialer: Arc<dyn TunnelDialer>,
    conns: AtomicU32,
    audit: super::audit::AuditLog,
}

impl ProxyCtx {
    pub fn active_conns(&self) -> u32 {
        self.conns.load(Ordering::Relaxed)
    }

    /// 审计快照（tunnel_status 的 sessions 字段）。
    pub async fn audit_snapshot(&self) -> Vec<super::types::TunnelSessionAudit> {
        self.audit.snapshot().await
    }
}

/// 本地反代监听体。`bind` 只绑 127.0.0.1 字面量；`serve` 由调用方 spawn，
/// 停止 = abort 该任务（listener 无部分状态；活动连接自然排空）。
pub struct LocalProxy {
    listener: TcpListener,
    pub ctx: Arc<ProxyCtx>,
}

impl LocalProxy {
    /// 绑定 127.0.0.1:0（§19.3-1：禁 localhost/0.0.0.0/::1，port 0 OS 分配）。
    pub async fn bind(
        target_port: u16,
        peer_id: String,
        dialer: Arc<dyn TunnelDialer>,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        Ok(Self {
            listener,
            ctx: Arc::new(ProxyCtx {
                target_port,
                peer_id,
                dialer,
                conns: AtomicU32::new(0),
                audit: super::audit::AuditLog::default(),
            }),
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.listener.local_addr().expect("bound listener has addr")
    }

    /// accept 循环；非回环来源直接拒（防绑定面意外暴露）。
    pub async fn serve(self) {
        loop {
            match self.listener.accept().await {
                Ok((tcp, peer)) if peer.ip().is_loopback() => {
                    let ctx = Arc::clone(&self.ctx);
                    ctx.conns.fetch_add(1, Ordering::Relaxed);
                    tokio::spawn(async move {
                        if let Err(reason) = serve_conn(tcp, &ctx).await {
                            tracing::warn!(%reason, "tunnel proxy 连接处理失败");
                        }
                        ctx.conns.fetch_sub(1, Ordering::Relaxed);
                    });
                }
                Ok((_, peer)) => tracing::warn!(%peer, "tunnel proxy 拒绝非回环来源"),
                Err(err) => {
                    tracing::warn!(error = %err, "tunnel proxy accept 失败");
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }
    }
}

/// 单连接处理：读请求头 → 重写 → 开隧道（带审计）→ 转发。
async fn serve_conn(mut local: TcpStream, ctx: &ProxyCtx) -> Result<(), String> {
    let (raw, leftover) = read_head(&mut local)
        .await
        .map_err(|e| format!("读请求头失败: {e}"))?;
    let head = Head::parse(&raw).map_err(|e| format!("请求头解析失败: {e}"))?;
    let websocket = head.is_websocket_upgrade();
    let content_len = head.content_length()?;
    if !websocket && head.has_chunked_body() {
        return reject(&mut local, 501, "chunked request body not supported").await;
    }
    // 先校验后动作（§19.3-3）：解析/校验全过才建流开数据面。
    let ticket = TunnelTicket::new_for_target(ctx.target_port)?;
    let audit = Arc::new(ConnAudit::new(
        ticket.uid.clone(),
        ctx.peer_id.clone(),
        format!("127.0.0.1:{}", ctx.target_port),
    ));
    ctx.audit.push(Arc::clone(&audit)).await;
    let result = forward_conn(local, ctx, &head, websocket, content_len, leftover, &audit).await;
    match &result {
        Ok(()) => audit.finish("ok").await,
        Err(reason) => {
            audit.finish(&failure_code(reason)).await;
            tracing::warn!(session_id = %audit.uid, %reason, "tunnel proxy 会话失败");
        }
    }
    result
}

/// 票据/应答帧错误映射 audit outcome（§19.2 六值闭集，禁自造第四类）。
fn failure_code(reason: &str) -> String {
    for code in [
        "target_not_allowed",
        "bad_ticket",
        "busy",
        "dial_failed",
        "shutdown",
    ] {
        if reason.contains(code) {
            return code.to_string();
        }
    }
    // 显式 error 帧码面未匹配闭集或裸 IO 故障 → 按规范页 io 兜底。
    "io".to_string()
}

/// 头解析/校验后的数据面：开隧道 → 头重写 → 双向转发。
async fn forward_conn(
    mut local: TcpStream,
    ctx: &ProxyCtx,
    head: &Head,
    websocket: bool,
    content_len: u64,
    leftover: Vec<u8>,
    audit: &Arc<ConnAudit>,
) -> Result<(), String> {
    let mut tunnel =
        match open_tunnel(ctx.dialer.as_ref(), audit, FIRST_REPLY_TIMEOUT).await {
            Ok(stream) => stream,
            Err(err) => {
                let reason = format!("隧道开启失败: {err}");
                reject(&mut local, 502, &reason).await?;
                return Err(reason);
            }
        };
    let mut head = head.clone();
    head.rewrite_host(ctx.target_port);
    head.apply_hop_by_hop(websocket);
    tunnel
        .write_all(&head.to_wire())
        .await
        .map_err(|e| format!("请求头写入隧道失败: {e}"))?;
    if websocket {
        upgrade_path(local, tunnel, leftover).await
    } else {
        plain_path(local, tunnel, leftover, content_len, audit).await
    }
}

/// WebSocket/升级路径：请求头后即 101 应答头，随后裸字节双向泵。
async fn upgrade_path(
    mut local: TcpStream,
    mut tunnel: BoxedStream,
    leftover: Vec<u8>,
) -> Result<(), String> {
    if !leftover.is_empty() {
        tunnel
            .write_all(&leftover)
            .await
            .map_err(|e| format!("升级请求余量写入失败: {e}"))?;
    }
    let (raw, rest) = read_reply_head(&mut tunnel).await?;
    local
        .write_all(&raw)
        .await
        .map_err(|e| format!("应答头写回失败: {e}"))?;
    if !is_101(&raw) {
        // 升级被目标拒绝：应答头已透传，剩余 body 排干至 EOF 即终点。
        drain_reply(&mut tunnel, &mut local, rest, &fallback_audit()).await?;
        return Ok(());
    }
    if !rest.is_empty() {
        // 101 后首包已含 WS 帧字节：先落泵，再交裸字节双向泵。
        local
            .write_all(&rest)
            .await
            .map_err(|e| format!("升级余量写回失败: {e}"))?;
    }
    tokio::io::copy_bidirectional(&mut local, &mut tunnel)
        .await
        .map(|_| ())
        .map_err(|e| format!("升级裸泵失败: {e}"))
}

/// 普通路径：请求体精确转发 + 半关闭；响应侧读到 EOF 即终点。
async fn plain_path(
    local: TcpStream,
    tunnel: BoxedStream,
    leftover: Vec<u8>,
    content_len: u64,
    audit: &Arc<ConnAudit>,
) -> Result<(), String> {
    let (mut local_read, mut local_write) = tokio::io::split(local);
    let (mut tunnel_read, mut tunnel_write) = tokio::io::split(tunnel);
    // 请求方向独立成任务：响应可先于请求体完成（互不阻塞，契约 §4）。
    let audit_out = Arc::clone(audit);
    let up = tokio::spawn(async move {
        forward_exact(&mut local_read, &mut tunnel_write, leftover, content_len, &audit_out).await?;
        if let Err(e) = tunnel_write.shutdown().await {
            tracing::warn!(error = %e, "tunnel 请求方向半关闭失败");
        }
        Ok::<(), String>(())
    });
    let audit_in = Arc::clone(audit);
    let down = async {
        let (raw, rest) = read_reply_head(&mut tunnel_read).await?;
        local_write
            .write_all(&raw)
            .await
            .map_err(|e| format!("响应头写回失败: {e}"))?;
        drain_reply(&mut tunnel_read, &mut local_write, rest, &audit_in).await
    };
    let result = down.await;
    up.abort();
    result
}

/// 升级被拒路径的字节计数兜底（真实会话审计在 serve_conn 已入册）。
fn fallback_audit() -> Arc<ConnAudit> {
    Arc::new(ConnAudit::new("0".repeat(16), "-".into(), "-".into()))
}

/// 反代自产错误应答（非隧道透传面）：501/502 + 人话，供浏览器与截图观测。
async fn reject(local: &mut TcpStream, status: u16, reason: &str) -> Result<(), String> {
    let body = format!("p2p tunnel proxy error {status}: {reason}\n");
    let head = format!(
        "HTTP/1.1 {status} Err\r\nContent-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    local
        .write_all(head.as_bytes())
        .await
        .map_err(|e| format!("错误应答写回失败: {e}"))?;
    local
        .write_all(body.as_bytes())
        .await
        .map_err(|e| format!("错误应答写回失败: {e}"))?;
    local
        .shutdown()
        .await
        .map_err(|e| format!("错误应答收尾失败: {e}"))
}

#[cfg(test)]
#[path = "proxy_tests.rs"]
mod tests;
