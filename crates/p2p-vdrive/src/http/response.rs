//! HTTP 应答模型与写出：状态行、Content-Length/chunked 择一、恒 Connection: close。

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

/// 应答体：内存字节 / 流式（len 已知时定长）/ 声明长度但无体（HEAD）。
pub enum ResponseBody {
    Bytes(Vec<u8>),
    Stream {
        stream: Box<dyn AsyncRead + Unpin + Send>,
        len: Option<u64>,
    },
    /// 无体但 Content-Length 报该值（HEAD 语义）。
    Size(u64),
    Empty,
}

pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: ResponseBody,
}

impl HttpResponse {
    pub fn status(code: u16) -> Self {
        Self {
            status: code,
            headers: Vec::new(),
            body: ResponseBody::Empty,
        }
    }

    pub fn bytes(code: u16, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            status: code,
            headers: vec![("Content-Type".into(), content_type.into())],
            body: ResponseBody::Bytes(body),
        }
    }

    pub fn text(code: u16, body: &str) -> Self {
        Self::bytes(code, "text/plain; charset=utf-8", body.as_bytes().to_vec())
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn body(mut self, body: ResponseBody) -> Self {
        self.body = body;
        self
    }
}

fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        206 => "Partial Content",
        207 => "Multi-Status",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        412 => "Precondition Failed",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        423 => "Locked",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        507 => "Insufficient Storage",
        _ => "Response",
    }
}

pub(super) async fn write_response(
    wr: &mut (impl AsyncWrite + Unpin),
    resp: HttpResponse,
) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {} {}\r\n", resp.status, reason(resp.status));
    let content_type_header = resp
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.clone());
    for (k, v) in &resp.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    let mut chunk_buf: Option<Vec<u8>> = None;
    let mut stream_len: Option<u64> = None;
    let mut stream: Option<Box<dyn AsyncRead + Unpin + Send>> = None;
    let mut declared_size: Option<u64> = None;
    match resp.body {
        ResponseBody::Bytes(bytes) => chunk_buf = Some(bytes),
        ResponseBody::Stream { stream: s, len } => {
            stream_len = len;
            stream = Some(s);
        }
        ResponseBody::Size(n) => declared_size = Some(n),
        ResponseBody::Empty => {}
    }
    if !chunk_buf.as_ref().map(|b| b.is_empty()).unwrap_or(false) && content_type_header.is_none() {
        head.push_str("Content-Type: application/octet-stream\r\n");
    }
    if let Some(bytes) = &chunk_buf {
        head.push_str(&format!("Content-Length: {}\r\n", bytes.len()));
    } else if let Some(len) = stream_len {
        head.push_str(&format!("Content-Length: {len}\r\n"));
    } else if let Some(n) = declared_size {
        head.push_str(&format!("Content-Length: {n}\r\n"));
    } else if chunk_buf.is_none() && stream.is_none() {
        head.push_str("Content-Length: 0\r\n");
    } else {
        head.push_str("Transfer-Encoding: chunked\r\n");
    }
    head.push_str("Connection: close\r\n\r\n");
    wr.write_all(head.as_bytes()).await?;
    match (chunk_buf, stream) {
        (Some(bytes), _) => wr.write_all(&bytes).await?,
        (None, Some(mut s)) => {
            tokio::io::copy(&mut s, wr).await?;
        }
        (None, None) => {}
    }
    wr.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_phrases_cover_dav() {
        assert_eq!(reason(207), "Multi-Status");
        assert_eq!(reason(204), "No Content");
        assert_eq!(reason(412), "Precondition Failed");
    }

    #[test]
    fn head_response_declares_size_without_body() {
        let resp = HttpResponse::status(200).body(ResponseBody::Size(42));
        assert!(matches!(resp.body, ResponseBody::Size(42)));
    }
}
