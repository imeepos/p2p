//! HTTP 报文头读写（契约 §6 兼容面 / §3.3 重写规则 / §19.3-2）：签名桩。
//! Host/Origin/Referer 重写 + hop-by-hop 取舍 + 请求头解析面；只切头不解释体，
//! 头后字节必须原样透传（含 leftover，禁缓冲丢失）。实现 = W-TB（迁移自
//! apps/gui/src-tauri/src/tunnel/head.rs，纯函数无外部依赖）。

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
        let _ = bytes;
        todo!("W-TB")
    }

    /// 按名取头（ASCII 大小写不敏感），多值取首个。出处：head.rs:41-46。
    pub fn header(&self, name: &str) -> Option<&str> {
        let _ = name;
        todo!("W-TB")
    }

    /// 覆写或追加头（保持其余头原样）。出处：head.rs:48-58。
    pub fn set_header(&mut self, name: &str, value: &str) {
        let _ = (name, value);
        todo!("W-TB")
    }

    /// 契约 §3.3：重写 Host 为目标 `127.0.0.1:<port>`；同面的 Origin/Referer
    /// 存在且 authority host 为回环字面量时重写为同一目标 authority（来源否则
    /// 停在反代端口上，被目标按 authority 校验拒绝写请求与 WS）；无该头不造头，
    /// 非回环/`null` 原样保留；重写目标仅限票据 target 的回环 authority，
    /// 不扩大信任面。出处：head.rs:60-74。
    pub fn rewrite_host(&mut self, target_port: u16) {
        let _ = target_port;
        todo!("W-TB")
    }

    /// hop-by-hop 取舍：非升级请求把 Connection 改为 close（一条浏览器连接对应
    /// 一条隧道流，响应终点 = 隧道 EOF）并剥 Keep-Alive/Proxy-Connection；
    /// WebSocket 升级请求原样保留 Connection/Upgrade（101 后是裸字节面）。
    /// 出处：head.rs:76-87。
    pub fn apply_hop_by_hop(&mut self, websocket: bool) {
        let _ = websocket;
        todo!("W-TB")
    }

    /// WebSocket 升级判定：Upgrade: websocket（大小写不敏感）且 Connection 含
    /// upgrade。出处：head.rs:89-95。
    pub fn is_websocket_upgrade(&self) -> bool {
        todo!("W-TB")
    }

    /// Content-Length（请求体转发量）；无头即 0，非法值为 Err。
    /// 出处：head.rs:97-106。
    pub fn content_length(&self) -> Result<u64, String> {
        todo!("W-TB")
    }

    /// 请求体是否 chunked（本反代不支持 chunked 请求体 → 501 拒）。
    /// 出处：head.rs:108-111。
    pub fn has_chunked_body(&self) -> bool {
        todo!("W-TB")
    }

    /// 序列化回 wire 字节（头尾 CRLF 完整）。出处：head.rs:113-126。
    pub fn to_wire(&self) -> Vec<u8> {
        todo!("W-TB")
    }
}

/// 读到 `\r\n\r\n` 为止的报文头；返回（原始头字节, 头后已到的 body 字节）。
/// 原始字节透传保证逐字保真（重序列化可能改变头行空格）；leftover 禁缓冲丢失；
/// 超 [HEAD_MAX] 显式报错（InvalidData），EOF 即 Err（UnexpectedEof）。
/// 出处：head.rs:129-156。
pub async fn read_head(
    stream: &mut (impl tokio::io::AsyncRead + Unpin),
) -> std::io::Result<(Vec<u8>, Vec<u8>)> {
    let _ = stream;
    todo!("W-TB")
}
