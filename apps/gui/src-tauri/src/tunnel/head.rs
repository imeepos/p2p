//! HTTP 报文头读写（冻结契约 §6：Host/Origin/Referer 重写 + hop-by-hop 取舍）。
//! 只切头不解释体：头后字节必须原样透传（含 leftover，禁缓冲丢失）。

use tokio::io::{AsyncRead, AsyncReadExt};

/// 报文头上限：浏览器请求头与 DSH 响应头都在数 KiB 量级，32 KiB 足够宽。
pub const HEAD_MAX: usize = 32 * 1024;

/// 解析后的 HTTP 头（首行 + 头键值对，键保留原样大小写）。
#[derive(Clone, Debug)]
pub struct Head {
    pub first_line: String,
    pub headers: Vec<(String, String)>,
}

impl Head {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|_| "报文头非 UTF-8".to_string())?;
        let mut lines = text.split("\r\n");
        let first_line = lines
            .next()
            .filter(|l| !l.is_empty())
            .ok_or("报文头首行为空")?
            .to_string();
        let mut headers = Vec::new();
        for line in lines {
            if line.is_empty() {
                break;
            }
            let (key, value) = line
                .split_once(':')
                .ok_or_else(|| format!("头行缺冒号: {line}"))?;
            headers.push((key.trim().to_string(), value.trim().to_string()));
        }
        Ok(Self {
            first_line,
            headers,
        })
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// 覆写或追加头（保持其余头原样）。
    pub fn set_header(&mut self, name: &str, value: &str) {
        match self
            .headers
            .iter_mut()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
        {
            Some(slot) => slot.1 = value.to_string(),
            None => self.headers.push((name.to_string(), value.to_string())),
        }
    }

    /// 冻结契约 §6：重写 Host 为目标 `127.0.0.1:<port>`；同面的 Origin/Referer
    /// 存在且 host 为回环字面量时改写为同一目标 authority——浏览器来源否则停在
    /// 反代端口上，被 DSH 按 authority 校验拒绝写请求与 WS（W-T3b 判别实验定案）。
    /// 无该头不造头。安全边界：重写目标仅限票据 target 的回环 authority，不扩大
    /// 信任面（非回环来源 authority 原样透传，由目标自行裁决）。
    pub fn rewrite_host(&mut self, target_port: u16) {
        self.set_header("Host", &format!("127.0.0.1:{target_port}"));
        for name in ["Origin", "Referer"] {
            if let Some(value) = self.header(name).map(str::to_string) {
                if let Some(rewritten) = rewrite_loopback_url(&value, target_port) {
                    self.set_header(name, &rewritten);
                }
            }
        }
    }

    /// hop-by-hop 取舍（记录在案）：非升级请求把 Connection 改为 close——
    /// 一条浏览器连接对应一条隧道流，响应终点 = 隧道 EOF，无需复用语义；
    /// WebSocket 升级请求原样保留 Connection/Upgrade（101 后是裸字节面）。
    pub fn apply_hop_by_hop(&mut self, websocket: bool) {
        if websocket {
            return;
        }
        self.set_header("Connection", "close");
        self.headers.retain(|(k, _)| {
            !k.eq_ignore_ascii_case("Keep-Alive") && !k.eq_ignore_ascii_case("Proxy-Connection")
        });
    }

    pub fn is_websocket_upgrade(&self) -> bool {
        self.header("Upgrade")
            .is_some_and(|v| v.eq_ignore_ascii_case("websocket"))
            && self
                .header("Connection")
                .is_some_and(|v| v.to_ascii_lowercase().contains("upgrade"))
    }

    /// Content-Length（请求体转发量）；无头即 0。
    pub fn content_length(&self) -> Result<u64, String> {
        match self.header("Content-Length") {
            None => Ok(0),
            Some(raw) => raw
                .trim()
                .parse()
                .map_err(|_| format!("Content-Length 非法: {raw}")),
        }
    }

    pub fn has_chunked_body(&self) -> bool {
        self.header("Transfer-Encoding")
            .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"))
    }

    /// 序列化回 wire 字节（头尾 CRLF 完整）。
    pub fn to_wire(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(self.first_line.as_bytes());
        out.extend_from_slice(b"\r\n");
        for (key, value) in &self.headers {
            out.extend_from_slice(key.as_bytes());
            out.extend_from_slice(b": ");
            out.extend_from_slice(value.as_bytes());
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"\r\n");
        out
    }
}

/// 读到 `\r\n\r\n` 为止的报文头；返回（原始头字节, 头后已到的 body 字节）。
/// 原始字节透传保证响应侧逐字保真（重序列化可能改变头行空格）；头后已到的
/// body 字节原样返回（leftover，禁缓冲丢失）。超 HEAD_MAX 显式报错（防失控）。
pub async fn read_head(
    stream: &mut (impl AsyncRead + Unpin),
) -> std::io::Result<(Vec<u8>, Vec<u8>)> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    loop {
        if let Some(pos) = find_head_end(&buf) {
            return Ok((buf[..pos + 4].to_vec(), buf[pos + 4..].to_vec()));
        }
        if buf.len() > HEAD_MAX {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "报文头超限",
            ));
        }
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "报文头不完整即 EOF",
            ));
        }
        buf.extend_from_slice(&chunk[..read]);
    }
}

fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

/// `scheme://host:port[/rest]` → `http://127.0.0.1:<port>[/rest]`；仅当 host 为
/// `127.0.0.1` 回环字面量（与 url.rs parse_dsh_url 同一信任判据）且端口可解析时
/// 改写。`null`、`localhost`、无端口等其余形态返回 None 原样保留；无头不造头
/// 的语义在 rewrite_host 侧（仅头已存在时才走到这里）。
fn rewrite_loopback_url(value: &str, target_port: u16) -> Option<String> {
    let (_, rest) = value.split_once("://")?;
    let authority_len = rest.find('/').unwrap_or(rest.len());
    let (host, port_raw) = rest[..authority_len].rsplit_once(':')?;
    if host != "127.0.0.1" || port_raw.parse::<u16>().is_err() {
        return None;
    }
    Some(format!(
        "http://127.0.0.1:{target_port}{}",
        &rest[authority_len..]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_rewrites_host() {
        let raw = b"GET /x HTTP/1.1\r\nHost: 127.0.0.1:40001\r\nAccept: */*\r\n\r\nbody!";
        let pos = find_head_end(raw).expect("head end");
        let mut head = Head::parse(&raw[..pos]).expect("parse");
        assert_eq!(head.first_line, "GET /x HTTP/1.1");
        assert_eq!(head.header("host"), Some("127.0.0.1:40001"));
        head.rewrite_host(3080);
        assert_eq!(head.header("Host"), Some("127.0.0.1:3080"));
        assert_eq!(head.header("Accept"), Some("*/*"));
        assert_eq!(head.content_length().unwrap(), 0);
        assert!(!head.is_websocket_upgrade());
    }

    #[test]
    fn origin_referer_rewrite_to_target_authority() {
        let raw = "POST /api/x HTTP/1.1\r\nHost: 127.0.0.1:40001\r\n\
                   Origin: http://127.0.0.1:52172\r\n\
                   Referer: http://127.0.0.1:52172/?token=t\r\n\r\n";
        let mut head = Head::parse(raw.as_bytes()).expect("parse");
        head.rewrite_host(3080);
        assert_eq!(head.header("Origin"), Some("http://127.0.0.1:3080"));
        assert_eq!(
            head.header("Referer"),
            Some("http://127.0.0.1:3080/?token=t")
        );
    }

    #[test]
    fn ws_upgrade_origin_rewritten_too() {
        let raw = "GET /api/remote.mux HTTP/1.1\r\nHost: a\r\nUpgrade: WebSocket\r\n\
                   Connection: Upgrade\r\nOrigin: http://127.0.0.1:52172\r\n\r\n";
        let mut head = Head::parse(raw.as_bytes()).expect("parse");
        assert!(head.is_websocket_upgrade());
        head.rewrite_host(3080);
        assert_eq!(head.header("Origin"), Some("http://127.0.0.1:3080"));
    }

    #[test]
    fn origin_referer_left_alone_when_absent_or_foreign() {
        let raw = "GET / HTTP/1.1\r\nHost: h\r\nOrigin: null\r\n\
                   Referer: http://localhost:9/p\r\n\r\n";
        let mut head = Head::parse(raw.as_bytes()).expect("parse");
        head.rewrite_host(3080);
        assert_eq!(head.header("Origin"), Some("null"));
        assert_eq!(head.header("Referer"), Some("http://localhost:9/p"));
        let mut bare = Head::parse(b"GET / HTTP/1.1\r\nHost: h\r\n\r\n").expect("parse");
        bare.rewrite_host(3080);
        assert!(bare.header("Origin").is_none());
        assert!(bare.header("Referer").is_none());
    }

    #[test]
    fn websocket_detection_and_hop_by_hop() {
        let raw = "GET /api/remote.mux HTTP/1.1\r\nHost: a\r\nUpgrade: WebSocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: x\r\n\r\n";
        let mut head = Head::parse(raw.as_bytes()).expect("parse");
        assert!(head.is_websocket_upgrade());
        head.apply_hop_by_hop(true);
        assert_eq!(head.header("Connection"), Some("Upgrade"));
        head.apply_hop_by_hop(false);
        assert_eq!(head.header("Connection"), Some("close"));
        assert!(head.header("Keep-Alive").is_none());
    }

    #[test]
    fn wire_round_trip_and_leftover_split() {
        let raw = b"POST /p HTTP/1.1\r\nHost: h\r\nContent-Length: 5\r\n\r\nhello";
        let pos = find_head_end(raw).expect("head end");
        let head = Head::parse(&raw[..pos]).expect("parse");
        assert_eq!(head.content_length().unwrap(), 5);
        assert_eq!(&raw[pos + 4..], b"hello");
        let wire = head.to_wire();
        assert!(wire.ends_with(b"\r\n\r\n"));
        assert!(std::str::from_utf8(&wire)
            .unwrap()
            .starts_with("POST /p HTTP/1.1\r\nHost: h\r\nContent-Length: 5\r\n\r\n"));
    }

    #[test]
    fn chunked_flag_and_bad_length() {
        let mut head =
            Head::parse(b"POST / HTTP/1.1\r\nHost: h\r\nTransfer-Encoding: chunked\r\n\r\n")
                .expect("parse");
        assert!(head.has_chunked_body());
        head.set_header("Content-Length", "abc");
        assert!(head.content_length().is_err());
    }
}
