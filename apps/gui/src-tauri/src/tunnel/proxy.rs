//! 本地回环反代（gui-contract §19.3 / 冻结契约 §5-§6）：只绑 `127.0.0.1`
//! 字面量，端口 0 由 OS 分配；一条浏览器连接 ↔ 一条隧道流（acp-pump
//! 「一连接一泵」体例）。每请求重写 Host 与同面 Origin/Referer 为目标
//! `127.0.0.1:<port>`（写请求/WS 的来源 authority 不改写会被目标栅栏拒）；非升级
//! 请求 Connection: close（hop-by-hop 取舍：响应终点=隧道 EOF，免复用语义）；
//! WebSocket 升级 101 后与隧道流裸字节双向泵；响应与 body 逐块转发禁整包
//! 缓冲。每条隧道流一条审计（audit::ConnAudit，uid 同源两侧日志）。

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use p2p_tunnel::{TunnelError, TunnelIo};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

use super::audit::ConnAudit;
use super::head::{read_head, Head};
use super::pump::{drain_reply, forward_exact, is_101, read_reply_head};

/// 开隧道缝：产出 ack 后的裸字节流（生产=TunnelClient 包装；单测=自足
/// 握手的直连裸流）。独立 trait 使反代测试免于耦合 TunnelClient 内部。
#[async_trait::async_trait]
pub trait TunnelOpener: Send + Sync {
    async fn open(&self, uid: &str, target: &str) -> Result<TunnelIo, TunnelError>;
}

/// 反代共享上下文。
pub struct ProxyCtx {
    target_port: u16,
    peer_id: String,
    opener: Arc<dyn TunnelOpener>,
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
        opener: Arc<dyn TunnelOpener>,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        Ok(Self {
            listener,
            ctx: Arc::new(ProxyCtx {
                target_port,
                peer_id,
                opener,
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
    eprintln!("[dbg] serve_conn entered");
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
    let audit = Arc::new(ConnAudit::new(
        new_uid()?,
        ctx.peer_id.clone(),
        format!("127.0.0.1:{}", ctx.target_port),
    ));
    ctx.audit.push(Arc::clone(&audit)).await;
    let result = forward_conn(local, ctx, &head, websocket, content_len, leftover, &audit).await;
    match &result {
        Ok(()) => audit.finish("ok").await,
        Err(reason) => {
            audit.finish("io").await;
            tracing::warn!(session_id = %audit.uid, %reason, "tunnel proxy 会话失败");
        }
    }
    result
}

/// TunnelError → audit outcome（§19.2 六值闭集，禁自造第四类）。
fn failure_code(err: &TunnelError) -> String {
    match err {
        TunnelError::Rejected { code, .. } => code.as_str().to_string(),
        TunnelError::Io(_) => "io".to_string(),
    }
}

/// 会话 uid（16 hex，两侧日志同源）。
fn new_uid() -> Result<String, String> {
    let mut bytes = [0u8; 8];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("uid 随机源失败: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
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
    let target = format!("127.0.0.1:{}", ctx.target_port);
    let mut tunnel = match ctx.opener.open(&audit.uid, &target).await {
        Ok(stream) => stream,
        Err(err) => {
            audit.finish(&failure_code(&err)).await;
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
    mut tunnel: TunnelIo,
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
/// 顺序化处理（请求头后浏览器必然等待响应，无双向并发需求；WS 升级的
/// 双向并发在 upgrade_path 的裸泵里），省 split/spawn 结构复杂度。
async fn plain_path(
    mut local: TcpStream,
    mut tunnel: TunnelIo,
    leftover: Vec<u8>,
    content_len: u64,
    audit: &Arc<ConnAudit>,
) -> Result<(), String> {
    forward_exact(&mut local, &mut tunnel, leftover, content_len, audit).await?;
    // 半关时机：响应收完再 FIN。DSH 按 Content-Length 判定请求完整即响应，
    // 不依赖 FIN；若在响应前 FIN，pump 的 wire 半关与响应回程存在时序冲突
    // （实证：FIN 后 target 写的帧泵侧读不到，见 2026-09-11 调试记录）。
    let (raw, rest) = read_reply_head(&mut tunnel).await?;
    local
        .write_all(&raw)
        .await
        .map_err(|e| format!("响应头写回失败: {e}"))?;
    drain_reply(&mut tunnel, &mut local, rest, audit).await?;
    if let Err(e) = tunnel.shutdown().await {
        tracing::warn!(error = %e, "tunnel 请求方向收尾关闭失败");
    }
    Ok(())
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
