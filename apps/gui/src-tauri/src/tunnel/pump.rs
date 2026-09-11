//! 隧道数据泵：请求体精确转发（bytesOut）与响应逐块排干（bytesIn）。
//! 流式转发禁整包缓冲；读侧容忍任意 ≤1 MiB 分块边界（冻结契约 §1/§4）。

use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::audit::ConnAudit;

/// 隧道 IO 空闲护栏：响应/长连接期间读停滞超过该值即放弃（禁无限静默悬挂）。
pub const TUNNEL_IDLE_GRACE: Duration = Duration::from_secs(30);

/// 读应答头原始字节（带空闲护栏）+ 头后已到的 body 字节。
pub async fn read_reply_head(
    stream: &mut (impl AsyncRead + Unpin),
) -> Result<(Vec<u8>, Vec<u8>), String> {
    tokio::time::timeout(TUNNEL_IDLE_GRACE, super::head::read_head(stream))
        .await
        .map_err(|_| "应答头等待超时".to_string())?
        .map_err(|e| format!("应答头读取失败: {e}"))
}

pub fn is_101(raw: &[u8]) -> bool {
    std::str::from_utf8(raw)
        .map(|t| t.starts_with("HTTP/1.1 101") || t.starts_with("HTTP/1.0 101"))
        .unwrap_or(false)
}

/// 请求体精确转发：leftover 先落，再补足 Content-Length 差额；逐块计 bytesOut。
pub async fn forward_exact(
    src: &mut (impl AsyncRead + Unpin + Send),
    dst: &mut (impl AsyncWrite + Unpin + Send),
    leftover: Vec<u8>,
    content_len: u64,
    audit: &ConnAudit,
) -> Result<(), String> {
    let mut remaining = content_len.saturating_sub(leftover.len() as u64);
    if !leftover.is_empty() {
        audit.add_out(leftover.len() as u64);
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
        audit.add_out(read as u64);
        dst.write_all(&chunk[..read])
            .await
            .map_err(|e| format!("请求体写入失败: {e}"))?;
        remaining -= read as u64;
    }
    Ok(())
}

/// 隧道 → 本地逐块排干（流式，禁整包缓冲）；EOF 即响应终点并关本地写半；
/// 逐块计 bytesIn。
pub async fn drain_reply(
    tunnel: &mut (impl AsyncRead + Unpin),
    local: &mut (impl AsyncWrite + Unpin),
    first: Vec<u8>,
    audit: &ConnAudit,
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
            audit.add_in(read as u64);
            pending = chunk[..read].to_vec();
        }
        local
            .write_all(&pending)
            .await
            .map_err(|e| format!("响应写回失败: {e}"))?;
        pending.clear();
    }
}
