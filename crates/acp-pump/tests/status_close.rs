//! status 服务收尾排空回归（2026-09-08 share_connect 并行假红实证）：
//! 401 等未读 body 即回的路径，若带未读数据关 socket 会触发 RST，
//! 客户端可能在读已写出的响应时收到 ConnectionReset。修复 = 按
//! Content-Length 精确排空后再放连接；本测试用 2MB body 强制「关闭时
//! 必有未读数据」，断言响应完整可读且零 RST（fix 前高概率复现）。

use std::sync::Arc;
use std::time::Duration;

use acp_pump::discovery::DiscoveryHub;
use acp_pump::state::StatusHub;
use acp_pump::status::{StatusDeps, StatusServer};
use acp_pump::ticket::TicketStore;
use p2p::Node;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const TOKEN: &str = "status-close-regression-token";
/// 足够大以撑满 socket 缓冲：未读数据确定存在（fix 前必 RST 量级）。
const BODY: usize = 2 * 1024 * 1024;
/// 排空 2MB 在回环是微秒级；5s 护栏足够。
const STEP: Duration = Duration::from_secs(5);

async fn status_addr(tag: &str) -> (std::net::SocketAddr, tempdir::TempDir) {
    let dir = tempdir::TempDir::new(tag);
    let node = Arc::new(
        Node::builder()
            .mdns(false)
            .data_dir(dir.0.join("node-id"))
            .build()
            .await
            .expect("node build"),
    );
    let deps = StatusDeps {
        hub: Arc::new(StatusHub::new()),
        discovery: Arc::new(DiscoveryHub::default()),
        tickets: Arc::new(TicketStore::new(&dir.0)),
        window: Duration::from_secs(90),
        node,
        ws_addr: "127.0.0.1:1".parse().expect("addr"),
        ws_token: TOKEN.to_string(),
    };
    let server = StatusServer::start(0, TOKEN.to_string(), deps)
        .await
        .expect("status server");
    (server.addr, dir)
}

/// 带大 body 的未授权 POST：写全请求后不关写端直接读尽响应，
/// 断言 401 完整返回且不出现 ConnectionReset。
async fn post_unauthorized_large_body(addr: std::net::SocketAddr) {
    let mut s = tokio::net::TcpStream::connect(addr).await.expect("connect");
    let head = format!(
        "POST /connect-share HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: {BODY}\r\n\r\n"
    );
    s.write_all(head.as_bytes()).await.expect("write head");
    let chunk = vec![b'x'; 64 * 1024];
    for _ in 0..(BODY / chunk.len()) {
        s.write_all(&chunk).await.expect("write body chunk");
    }
    s.flush().await.expect("flush");
    let mut resp = Vec::new();
    tokio::time::timeout(STEP, s.read_to_end(&mut resp))
        .await
        .expect("read response within step")
        .expect("读响应不得出现连接重置（RST 竞掉已写出响应）");
    let text = String::from_utf8_lossy(&resp);
    assert!(
        text.starts_with("HTTP/1.1 401"),
        "应完整读到 401 响应，实得: {text}"
    );
}

#[tokio::test]
async fn unauthorized_post_with_unread_body_reads_full_response() {
    let (addr, _dir) = status_addr("status-close").await;
    for i in 0..3 {
        post_unauthorized_large_body(addr).await;
        println!("iteration {i}: full 401 response read, no reset");
    }
}

/// 测试目录守卫（退出即清理，失败时留路径供排查）。
mod tempdir {
    use std::path::PathBuf;

    pub(super) struct TempDir(pub PathBuf);

    impl TempDir {
        pub fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "acp-pump-status-close-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create temp dir");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
