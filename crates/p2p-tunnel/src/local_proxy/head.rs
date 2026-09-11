//! HTTP 报文头读写（契约 §6 兼容面 / §3.3 重写规则 / §19.3-2）。
//! Host/Origin/Referer 重写 + hop-by-hop 取舍 + 请求头解析面；只切头不解释体，
//! 头后字节必须原样透传（含 leftover，禁缓冲丢失）。实现迁移自
//! apps/gui/src-tauri/src/tunnel/head.rs（纯函数无外部依赖）。

use tokio::io::AsyncReadExt;

/// 报文头上限：浏览器请求头与响应头都在数 KiB 量级，32 KiB 足够宽；超限显式
/// 报错防失控。出处：apps/gui/src-tauri/src/tunnel/head.rs:6-7。
pub const HEAD_MAX: usize = 32 * 1024;

/// 解析后的 HTTP 头（首行 + 头键值对，键保留原样大小写）。
/// 出处：apps/gui/src-tauri/src/tunnel/head.rs:9-14。
#[derive(Clone, Debug)]
pub struct Head {
    pub first_line: String,
    pub headers: Vec<(String, String)>,
}

impl Head {
    /// 解析原始头字节（不含头后 body）：首行非空 + 头行 `key: value`；
    /// 非 UTF-8 / 空首行 / 缺冒号为 Err。出处：head.rs:16-39。
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

    /// 按名取头（ASCII 大小写不敏感），多值取首个。出处：head.rs:41-46。
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// 覆写或追加头（保持其余头原样）。出处：head.rs:48-58。
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

    /// 契约 §3.3：重写 Host 为目标 `127.0.0.1:<port>`；同面的 Origin/Referer
    /// 存在且 authority host 为回环字面量时重写为同一目标 authority（来源否则
    /// 停在反代端口上，被目标按 authority 校验拒绝写请求与 WS）；无该头不造头，
    /// 非回环/`null` 原样保留；重写目标仅限票据 target 的回环 authority，
    /// 不扩大信任面。出处：head.rs:60-74。
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

    /// hop-by-hop 取舍：非升级请求把 Connection 改为 close（一条浏览器连接对应
    /// 一条隧道流，响应终点 = 隧道 EOF）并剥 Keep-Alive/Proxy-Connection；
    /// WebSocket 升级请求原样保留 Connection/Upgrade（101 后是裸字节面）。
    /// 出处：head.rs:76-87。
    pub fn apply_hop_by_hop(&mut self, websocket: bool) {
        if websocket {
            return;
        }
        self.set_header("Connection", "close");
        self.headers.retain(|(k, _)| {
            !k.eq_ignore_ascii_case("Keep-Alive") && !k.eq_ignore_ascii_case("Proxy-Connection")
        });
    }

    /// WebSocket 升级判定：Upgrade: websocket（大小写不敏感）且 Connection 含
    /// upgrade。出处：head.rs:89-95。
    pub fn is_websocket_upgrade(&self) -> bool {
        self.header("Upgrade")
            .is_some_and(|v| v.eq_ignore_ascii_case("websocket"))
            && self
                .header("Connection")
                .is_some_and(|v| v.to_ascii_lowercase().contains("upgrade"))
    }

    /// Content-Length（请求体转发量）；无头即 0，非法值为 Err。
    /// 出处：head.rs:97-106。
    pub fn content_length(&self) -> Result<u64, String> {
        match self.header("Content-Length") {
            None => Ok(0),
            Some(raw) => raw
                .trim()
                .parse()
                .map_err(|_| format!("Content-Length 非法: {raw}")),
        }
    }

    /// 请求体是否 chunked（本反代不支持 chunked 请求体 → 501 拒）。
    /// 出处：head.rs:108-111。
    pub fn has_chunked_body(&self) -> bool {
        self.header("Transfer-Encoding")
            .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"))
    }

    /// 序列化回 wire 字节（头尾 CRLF 完整）。出处：head.rs:113-126。
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
/// 原始字节透传保证逐字保真（重序列化可能改变头行空格）；leftover 禁缓冲丢失；
/// 超 [HEAD_MAX] 显式报错（InvalidData），EOF 即 Err（UnexpectedEof）。
/// 出处：head.rs:129-156。
pub async fn read_head(
    stream: &mut (impl tokio::io::AsyncRead + Unpin),
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
#[path = "head_tests.rs"]
mod tests;
