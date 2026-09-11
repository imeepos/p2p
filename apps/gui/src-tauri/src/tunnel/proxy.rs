//! 本地回环反代（冻结契约 §5/§6/§7）：只绑 `127.0.0.1` 字面量，端口 0 由
//! OS 分配；一条浏览器连接 ↔ 一条隧道流（acp-pump「一连接一泵」体例）。
//! 每请求重写 Host 为目标 `127.0.0.1:<port>`；非升级请求 Connection: close
//! （hop-by-hop 取舍：响应终点 = 隧道 EOF，免复用语义）；WebSocket 升级在
//! 101 后与隧道流做裸字节双向泵。响应与 body 逐块转发，禁整包缓冲。

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use p2p::BoxedStream;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::client::{open_tunnel, TunnelDialer, FIRST_REPLY_TIMEOUT};
use super::head::{read_head, Head};
use super::ticket::TunnelTicket;

/// 隧道 IO 空闲护栏：响应/长连接期间读停滞超过该值即放弃（禁无限静默悬挂；
/// 读侧容忍任意 ≤1 MiB 分块边界，护栏远大于正常静默间隔）。
const TUNNEL_IDLE_GRACE: Duration = Duration::from_secs(30);

/// 反代共享上下文。
pub struct ProxyCtx {
    target_port: u16,
    dialer: Arc<dyn TunnelDialer>,
    conns: AtomicU32,
}

impl ProxyCtx {
    pub fn active_conns(&self) -> u32 {
        self.conns.load(Ordering::Relaxed)
    }
}

/// 本地反代监听体。`bind` 只绑 127.0.0.1 字面量；`serve` 由调用方 spawn，
/// 停止 = abort 该任务（listener 无部分状态；活动连接自然排空）。
pub struct LocalProxy {
    listener: TcpListener,
    pub ctx: Arc<ProxyCtx>,
}

impl LocalProxy {
    /// 绑定 127.0.0.1:0（冻结契约 §5：不用 localhost，port 0 由 OS 分配）。
    pub async fn bind(
        target_port: u16,
        dialer: Arc<dyn TunnelDialer>,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        Ok(Self {
            listener,
            ctx: Arc::new(ProxyCtx {
                target_port,
                dialer,
                conns: AtomicU32::new(0),
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

/// 单连接处理：读请求头 → 重写 → 开隧道 → 转发。
async fn serve_conn(mut local: TcpStream, ctx: &ProxyCtx) -> Result<(), String> {
    let (raw, leftover) = read_head(&mut local)
        .await
        .map_err(|e| format!("读请求头失败: {e}"))?;
    let mut head = Head::parse(&raw).map_err(|e| format!("请求头解析失败: {e}"))?;
    let websocket = head.is_websocket_upgrade();
    let content_len = head.content_length()?;
    if !websocket && head.has_chunked_body() {
        return reject(&mut local, 501, "chunked request body not supported").await;
    }
    let ticket = TunnelTicket::new_for_target(ctx.target_port)?;
    let mut tunnel = match open_tunnel(ctx.dialer.as_ref(), &ticket, FIRST_REPLY_TIMEOUT).await {
        Ok(stream) => stream,
        Err(err) => {
            let reason = format!("隧道开启失败: {err}");
            reject(&mut local, 502, &reason).await?;
            return Err(reason);
        }
    };
    head.rewrite_host(ctx.target_port);
    head.apply_hop_by_hop(websocket);
    tunnel
        .write_all(&head.to_wire())
        .await
        .map_err(|e| format!("请求头写入隧道失败: {e}"))?;
    if websocket {
        upgrade_path(local, tunnel, leftover).await
    } else {
        plain_path(local, tunnel, leftover, content_len).await
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
        drain_reply(&mut tunnel, &mut local, rest).await?;
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
) -> Result<(), String> {
    let (mut local_read, mut local_write) = tokio::io::split(local);
    let (mut tunnel_read, mut tunnel_write) = tokio::io::split(tunnel);
    // 请求方向独立成任务：响应可先于请求体完成（互不阻塞，契约 §4）。
    let up = tokio::spawn(async move {
        forward_exact(&mut local_read, &mut tunnel_write, leftover, content_len).await?;
        if let Err(e) = tunnel_write.shutdown().await {
            tracing::warn!(error = %e, "tunnel 请求方向半关闭失败");
        }
        Ok::<(), String>(())
    });
    let down = async {
        let (raw, rest) = read_reply_head(&mut tunnel_read).await?;
        local_write
            .write_all(&raw)
            .await
            .map_err(|e| format!("响应头写回失败: {e}"))?;
        drain_reply(&mut tunnel_read, &mut local_write, rest).await
    };
    let result = down.await;
    up.abort();
    result
}

/// 请求体精确转发：leftover 先落，再补足 Content-Length 差额。
async fn forward_exact(
    src: &mut (impl AsyncRead + Unpin + Send),
    dst: &mut (impl AsyncWrite + Unpin + Send),
    leftover: Vec<u8>,
    content_len: u64,
) -> Result<(), String> {
    let mut remaining = content_len.saturating_sub(leftover.len() as u64);
    if !leftover.is_empty() {
        dst.write_all(&leftover)
            .await
            .map_err(|e| format!("请求体写入失败: {e}"))?;
    }
    let mut chunk = vec![0u8; 64 * 1024];
    while remaining > 0 {
        let want = chunk.len().min(remaining as usize);
        let read = src
            .read(&mut chunk[..want])
            .await
            .map_err(|e| format!("请求体读取失败: {e}"))?;
        if read == 0 {
            return Err("请求体在 Content-Length 之前 EOF".into());
        }
        dst.write_all(&chunk[..read])
            .await
            .map_err(|e| format!("请求体写入失败: {e}"))?;
        remaining -= read as u64;
    }
    Ok(())
}

/// 读应答头原始字节（带空闲护栏）+ 头后已到的 body 字节。
async fn read_reply_head(
    stream: &mut (impl AsyncRead + Unpin),
) -> Result<(Vec<u8>, Vec<u8>), String> {
    tokio::time::timeout(TUNNEL_IDLE_GRACE, read_head(stream))
        .await
        .map_err(|_| "应答头等待超时".to_string())?
        .map_err(|e| format!("应答头读取失败: {e}"))
}

fn is_101(raw: &[u8]) -> bool {
    std::str::from_utf8(raw)
        .map(|t| t.starts_with("HTTP/1.1 101") || t.starts_with("HTTP/1.0 101"))
        .unwrap_or(false)
}

/// 隧道 → 本地逐块排干（流式，禁整包缓冲）；EOF 即响应终点并关本地写半。
async fn drain_reply(
    tunnel: &mut (impl AsyncRead + Unpin),
    local: &mut (impl AsyncWrite + Unpin),
    first: Vec<u8>,
) -> Result<(), String> {
    let mut pending = first;
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        if pending.is_empty() {
            let read = tokio::time::timeout(TUNNEL_IDLE_GRACE, tunnel.read(&mut chunk))
                .await
                .map_err(|_| "响应等待超时".to_string())?
                .map_err(|e| format!("响应读取失败: {e}"))?;
            if read == 0 {
                return local
                    .shutdown()
                    .await
                    .map_err(|e| format!("本地写半关闭失败: {e}"));
            }
            pending = chunk[..read].to_vec();
        }
        local
            .write_all(&pending)
            .await
            .map_err(|e| format!("响应写回失败: {e}"))?;
        pending.clear();
    }
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
