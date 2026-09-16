//! WebDAV Class 1 服务：把 HTTP/DAV 方法翻译成 [FsBackend] 操作。
//!
//! 覆盖挂载所需方法面：OPTIONS / PROPFIND / GET / HEAD / PUT / MKCOL /
//! DELETE / MOVE / COPY / PROPPATCH(形答) / LOCK(假持锁) / UNLOCK。
//! 兼容取舍见 specs/vdrive.md §6。

pub mod copymove;
pub mod get;
pub mod xml;

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;

use crate::backend::FsBackend;
use crate::error::ErrorKind;
use crate::http::{HttpRequest, HttpResponse, HttpHandler};
use copymove::{clear_dest, copy_tree, delete_tree, destination_vpath};
use get::status_for;

/// 挂载桥服务：backend 可以是 LocalFs（本机直挂）或 VDriveClient（远端挂载）。
pub struct DavService {
    backend: Arc<dyn FsBackend>,
}

impl DavService {
    pub fn new(backend: Arc<dyn FsBackend>) -> Self {
        Self { backend }
    }

    async fn propfind(&self, vpath: &str, depth: Option<&str>) -> HttpResponse {
        let depth = depth.unwrap_or("infinity").trim().to_ascii_lowercase();
        if depth == "infinity" {
            // RFC 4918 §9.1：无界深度的实现成本/风险不成比例，按规范显式 400。
            return HttpResponse::text(400, "Depth: infinity is not supported");
        }
        let entry = match self.backend.stat(vpath).await {
            Ok(e) => e,
            Err(e) => return status_for(&e),
        };
        let mut entries = vec![entry.clone()];
        if entry.kind == crate::wire::EntryKind::Dir && depth == "1" {
            match self.backend.list(vpath).await {
                Ok(children) => entries.extend(children),
                Err(e) => return status_for(&e),
            }
        }
        HttpResponse::bytes(
            207,
            "application/xml; charset=utf-8",
            xml::multistatus(&entries).into_bytes(),
        )
    }

    async fn mkcol(&self, vpath: &str) -> HttpResponse {
        if vpath == "/" {
            return HttpResponse::status(405);
        }
        match self.backend.mkdir(vpath).await {
            Ok(()) => HttpResponse::status(201),
            Err(e) if e.kind == ErrorKind::NotFound => HttpResponse::status(409),
            Err(e) => status_for(&e),
        }
    }

    async fn delete(&self, vpath: &str) -> HttpResponse {
        if vpath == "/" {
            return HttpResponse::status(403);
        }
        match delete_tree(&self.backend, vpath).await {
            Ok(()) => HttpResponse::status(204),
            Err(e) => status_for(&e),
        }
    }

    /// MOVE/COPY 共用：入参全部在 respond 期提取（避免跨 await 持请求引用）。
    async fn two_path_op(
        &self,
        src: &str,
        dest: Option<String>,
        overwrite: bool,
        is_move: bool,
    ) -> HttpResponse {
        let Some(dest) = dest else {
            return HttpResponse::text(400, "missing or bad Destination header");
        };
        if dest == "/" || dest == src {
            return HttpResponse::status(403);
        }
        let dest_existed = match clear_dest(&self.backend, &dest, overwrite).await {
            Ok(existed) => existed,
            Err(e) => return status_for(&e),
        };
        let outcome = if is_move {
            self.backend.rename(src, &dest).await
        } else {
            copy_tree(&self.backend, src, &dest).await
        };
        match outcome {
            Ok(()) => HttpResponse::status(if dest_existed { 204 } else { 201 }),
            Err(e) => status_for(&e),
        }
    }
}

#[async_trait]
impl HttpHandler for DavService {
    async fn respond(&self, mut req: HttpRequest) -> HttpResponse {
        let decoded = req.path();
        let Some(vpath) = crate::localfs::normalize_vpath(&decoded) else {
            return HttpResponse::text(400, "path escapes root");
        };
        let method = req.method().to_string();
        let dest = req.header("destination").and_then(destination_vpath);
        let overwrite = req
            .header("overwrite")
            .map(|v| v.trim().eq_ignore_ascii_case("t"))
            .unwrap_or(true);
        let depth = req.header("depth").map(str::to_string);
        match method.as_str() {
            "OPTIONS" => dav_options(),
            "PROPFIND" => {
                self.propfind(&vpath, depth.as_deref()).await
            }
            "PROPPATCH" => proppatch_stub(&vpath),
            "GET" => get::get(&self.backend, &vpath, false).await,
            "HEAD" => get::get(&self.backend, &vpath, true).await,
            "PUT" => get::put(&self.backend, &vpath, &mut req).await,
            "MKCOL" => self.mkcol(&vpath).await,
            "DELETE" => self.delete(&vpath).await,
            "MOVE" => self.two_path_op(&vpath, dest, overwrite, true).await,
            "COPY" => self.two_path_op(&vpath, dest, overwrite, false).await,
            "LOCK" => lock_stub(),
            "UNLOCK" => HttpResponse::status(204),
            _ => HttpResponse::status(405).with_header("Allow", DAV_METHODS),
        }
    }
}

const DAV_METHODS: &str = "OPTIONS, GET, HEAD, PUT, PROPFIND, PROPPATCH, MKCOL, DELETE, MOVE, COPY, LOCK, UNLOCK";

fn dav_options() -> HttpResponse {
    HttpResponse::status(200)
        .with_header("DAV", "1, 2")
        .with_header("MS-Author-Via", "DAV")
        .with_header("Allow", DAV_METHODS)
        .with_header("Content-Length", "0")
}

/// PROPPATCH：属性写入不支持，但回 207 全 200 保住客户端流程（Finder/Word
/// 会带 mtime 属性；后端 mtime 以系统语义为准，见 spec §6 兼容性）。
fn proppatch_stub(vpath: &str) -> HttpResponse {
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<D:multistatus xmlns:D=\"DAV:\">\
<D:response><D:href>{}</D:href><D:propstat><D:prop/>\
<D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response></D:multistatus>",
        xml::escape(vpath)
    );
    HttpResponse::bytes(207, "application/xml; charset=utf-8", body.into_bytes())
}

/// 假持锁：返回合法 lockdiscovery（token 即时生成不登记，桥不持锁）。
fn lock_stub() -> HttpResponse {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let token = format!("opaquelocktoken:vdrive-{nanos}");
    let body = xml::lock_response(&token, "infinity", "vdrive-bridge");
    HttpResponse::bytes(200, "application/xml; charset=utf-8", body.into_bytes())
        .with_header("Lock-Token", &format!("<{token}>"))
}
