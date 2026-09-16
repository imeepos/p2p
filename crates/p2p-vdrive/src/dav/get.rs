//! DAV 读写面：GET/HEAD（下行流式泵）与 PUT（上行流式），内存占用恒定。

use std::sync::Arc;

use tokio::io::AsyncWriteExt;

use crate::backend::FsBackend;
use crate::error::{ErrorKind, VDriveError};
use crate::http::{HttpRequest, HttpResponse, ResponseBody};
use crate::wire::EntryKind;

/// GET/HEAD：目录回 HTML 索引页；文件流式下行（Content-Length 来自 stat）。
pub async fn get(backend: &Arc<dyn FsBackend>, vpath: &str, head_only: bool) -> HttpResponse {
    let entry = match backend.stat(vpath).await {
        Ok(e) => e,
        Err(e) => return status_for(&e),
    };
    match entry.kind {
        EntryKind::Dir => dir_index(backend, vpath, head_only).await,
        EntryKind::File => file_response(backend, vpath, &entry, head_only),
    }
}

fn file_response(
    backend: &Arc<dyn FsBackend>,
    vpath: &str,
    entry: &crate::wire::Entry,
    head_only: bool,
) -> HttpResponse {
    let content_type = mime_of(vpath);
    if head_only {
        return HttpResponse::status(200)
            .with_header("Content-Type", content_type)
            .with_header("ETag", &format!("\"{}-{}\"", entry.size, entry.mtime))
            .body(ResponseBody::Size(entry.size));
    }
    let (stream, len) = pump(backend.clone(), vpath.to_string(), entry.size);
    HttpResponse::status(200)
        .with_header("Content-Type", content_type)
        .with_header("ETag", &format!("\"{}-{}\"", entry.size, entry.mtime))
        .body(ResponseBody::Stream {
            stream: Box::new(stream),
            len: Some(len),
        })
}

/// 下行泵：后端分块读 → duplex 流，桥接内存占用 = 一个 chunk。
fn pump(
    backend: Arc<dyn FsBackend>,
    vpath: String,
    size: u64,
) -> (tokio::io::DuplexStream, u64) {
    let (mut tx, rx) = tokio::io::duplex(crate::wire::MAX_CHUNK as usize * 2);
    tokio::spawn(async move {
        let mut offset = 0u64;
        while offset < size {
            let want = ((size - offset).min(crate::wire::MAX_CHUNK as u64)) as u32;
            match backend.read(&vpath, offset, want).await {
                Ok(bytes) if bytes.is_empty() => break,
                Ok(bytes) => {
                    offset += bytes.len() as u64;
                    if tx.write_all(&bytes).await.is_err() {
                        break; // 下行端已断开（客户端取消），泵自然结束
                    }
                }
                Err(e) => {
                    tracing::warn!(vpath = %vpath, "dav GET read failed mid-stream: {e}");
                    break; // 显式截断：客户端以短读感知失败，此处已发过 200 头
                }
            }
        }
        let _ = tx.flush().await;
        drop(tx);
    });
    (rx, size)
}

async fn dir_index(backend: &Arc<dyn FsBackend>, vpath: &str, head_only: bool) -> HttpResponse {
    let rows = match backend.list(vpath).await {
        Ok(items) => items,
        Err(e) => return status_for(&e),
    };
    let mut html = format!(
        "<html><head><meta charset=\"utf-8\"><title>Index of {vpath}</title></head><body>\
<h1>Index of {vpath}</h1><ul>"
    );
    for item in rows {
        let suffix = if item.kind == EntryKind::Dir { "/" } else { "" };
        html.push_str(&format!(
            "<li><a href=\"{}{}\">{}{}</a></li>",
            vpath.trim_end_matches('/'),
            format!("/{}{}", item.name, suffix).as_str(),
            item.name,
            suffix
        ));
    }
    html.push_str("</ul></body></html>");
    if head_only {
        return HttpResponse::status(200)
            .with_header("Content-Type", "text/html; charset=utf-8")
            .body(ResponseBody::Size(html.len() as u64));
    }
    HttpResponse::bytes(200, "text/html; charset=utf-8", html.into_bytes())
}

/// PUT：整体替换（存在先截断）或新建；流式上行，逐块定位写。
pub async fn put(
    backend: &Arc<dyn FsBackend>,
    vpath: &str,
    body: &mut HttpRequest,
) -> HttpResponse {
    let existed = match backend.stat(vpath).await {
        Ok(e) if e.kind == EntryKind::Dir => {
            return HttpResponse::status(409).with_header("Allow", "GET, HEAD, PROPFIND")
        }
        Ok(_) => true,
        Err(e) if e.kind == ErrorKind::NotFound => {
            if !parent_exists(backend, vpath).await {
                return HttpResponse::status(409); // 409 Conflict：父目录缺失
            }
            false
        }
        Err(e) => return status_for(&e),
    };
    if existed && backend.truncate(vpath, 0).await.is_err() {
        return HttpResponse::status(500);
    }
    let mut offset = 0u64;
    let mut chunk = vec![0u8; crate::wire::MAX_CHUNK as usize];
    loop {
        match body.body.fill(&mut chunk).await {
            Ok(0) => break,
            Ok(n) => {
                if let Err(e) = backend.write(vpath, offset, &chunk[..n]).await {
                    tracing::warn!(vpath = %vpath, "dav PUT write failed: {e}");
                    return status_for(&e);
                }
                offset += n as u64;
            }
            Err(e) => {
                tracing::warn!(vpath = %vpath, "dav PUT body read failed: {e}");
                return HttpResponse::status(400);
            }
        }
    }
    HttpResponse::status(if existed { 204 } else { 201 })
}

async fn parent_exists(backend: &Arc<dyn FsBackend>, vpath: &str) -> bool {
    match super::copymove::parent_vpath(vpath) {
        None => false,
        Some(parent) => matches!(
            backend.stat(&parent).await,
            Ok(e) if e.kind == EntryKind::Dir
        ),
    }
}

fn mime_of(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "txt" | "md" | "log" | "rs" | "toml" | "json" => "text/plain; charset=utf-8",
        "html" | "htm" => "text/html; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// VDriveError → HTTP 状态（WebDAV 语义）。
pub(crate) fn status_for(e: &VDriveError) -> HttpResponse {
    let code = match e.kind {
        ErrorKind::NotFound => 404,
        ErrorKind::AlreadyExists => 405,
        ErrorKind::NotEmpty => 409,
        ErrorKind::NotDir => 405,
        ErrorKind::PermissionDenied => 403,
        ErrorKind::InvalidPath => 400,
        ErrorKind::TooLarge => 413,
        ErrorKind::Unsupported => 501,
        ErrorKind::Io => 502,
    };
    HttpResponse::text(code, &e.msg)
}
